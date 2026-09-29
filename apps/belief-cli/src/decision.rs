//! Versioned, one-decision boundary. Validate and authorize before resolving a provider.
use crate::pipeline::derive_belief;
use belief_core::*;
use belief_policy::{PolicyConfig, POLICY_KEYS};
use belief_store::InMemoryBeliefStore;
use evidence_interchange::ValidatedEvidenceBatch;
use inference_core::{EvidenceUse, InferenceRequest, JudgmentBasis, TrustedInferenceRule};
use semantic_decision::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_INPUT_BYTES: usize = 1_048_576;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema: String,
    schema_version: u32,
    id: String,
    state: Value,
    question: String,
    options: Vec<OptionInput>,
    class: String,
    evidence: Value,
    evidence_uses: Vec<UseInput>,
    proposition: PropositionInput,
    judgment_spec: SpecInput,
    policy: BTreeMap<String, String>,
    provider: ProviderSelection,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionInput {
    id: String,
    description: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UseInput {
    id: String,
    purpose: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecInput {
    name: String,
    revision: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PropositionInput {
    subject: String,
    predicate: String,
    object: ObjectInput,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ObjectInput {
    Entity(String),
    Text(String),
    Boolean(bool),
}

/// The host controls supported provider resolution. Fixture execution is explicitly simulated.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderSelection {
    Semif { tier: String },
    Fixture { scores: BTreeMap<String, f64> },
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}
impl Failure {
    pub fn new(code: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            code,
            message: error.to_string(),
        }
    }
    pub fn report(&self) -> Value {
        json!({"schema":"belief_semantic_result", "schemaVersion":1, "status":"error", "error":self})
    }
}
fn invalid(error: impl std::fmt::Display) -> Failure {
    Failure::new("invalid_request", error)
}

/// `resolve` is never called for malformed or unauthorized inputs.
pub fn execute(
    input: &str,
    resolve: impl FnOnce(&ProviderSelection) -> Result<Box<dyn SemanticDecisionEngine>, Failure>,
) -> Result<Value, Failure> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(invalid("request exceeds 1 MiB"));
    }
    let original: Value = serde_json::from_str(input).map_err(invalid)?;
    let input: Input = serde_json::from_value(original.clone()).map_err(invalid)?;
    if input.schema != "belief_semantic_request" || input.schema_version != 1 {
        return Err(Failure::new(
            "unsupported_schema",
            "expected belief_semantic_request@1",
        ));
    }
    if input
        .policy
        .keys()
        .any(|key| !POLICY_KEYS.contains(&key.as_str()))
    {
        return Err(invalid("unknown policy key"));
    }
    let policy =
        PolicyConfig::from_pairs(input.policy.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .map_err(invalid)?;
    let batch = ValidatedEvidenceBatch::parse_json(&input.evidence.to_string()).map_err(invalid)?;
    let class =
        InferenceClass::parse(&input.class).ok_or_else(|| invalid("unknown inference class"))?;
    let proposition = Proposition::new(
        EntityId::new(input.proposition.subject).map_err(invalid)?,
        Predicate::new(input.proposition.predicate).map_err(invalid)?,
        match input.proposition.object {
            ObjectInput::Entity(id) => ObjectValue::Entity(EntityId::new(id).map_err(invalid)?),
            ObjectInput::Text(text) => ObjectValue::text(text).map_err(invalid)?,
            ObjectInput::Boolean(value) => ObjectValue::Boolean(value),
        },
    );
    let spec = JudgmentSpecRef::new(input.judgment_spec.name, input.judgment_spec.revision)
        .map_err(invalid)?;
    let mut seen = BTreeSet::new();
    let uses = input
        .evidence_uses
        .into_iter()
        .map(|item| {
            if !seen.insert(item.id.clone()) {
                return Err(invalid("duplicate evidence use"));
            }
            let evidence = batch
                .evidence()
                .iter()
                .find(|e| e.id.as_str() == item.id)
                .ok_or_else(|| invalid(format!("missing evidence {}", item.id)))?
                .clone();
            let purpose = EvidencePurpose::parse(&item.purpose)
                .ok_or_else(|| invalid("unknown evidence purpose"))?;
            Ok(DecisionEvidenceUse::new(evidence, purpose))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let options = input
        .options
        .into_iter()
        .map(|o| DecisionOption::new(o.id, o.description).map_err(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = BTreeSet::from([SUPPORTS_OPTION_ID, CONTRADICTS_OPTION_ID, UNKNOWN_OPTION_ID]);
    if options
        .iter()
        .map(DecisionOption::id)
        .collect::<BTreeSet<_>>()
        != expected
    {
        return Err(invalid(
            "requires supports, contradicts, and unknown options",
        ));
    }
    let request = DecisionRequest::new(input.id, input.state, input.question, options, class, uses)
        .map_err(invalid)?;
    let authorized = request
        .authorize(&policy)
        .map_err(|e| Failure::new("policy_denied", e))?;
    let batch = batch
        .authorize(&policy)
        .map_err(|e| Failure::new("policy_denied", e))?;
    let rule = TrustedInferenceRule::for_class(class)
        .ok_or_else(|| invalid("no trusted inference rule for class"))?;
    let engine = resolve(&input.provider)?;
    let receipt = engine
        .decide(&authorized)
        .map_err(|e| Failure::new("provider_failed", e))?;
    let id = authorized.request().id().to_owned();
    let evidence = authorized.request().evidence().to_vec();
    let judgment = authorized
        .into_three_way_judgment(
            receipt,
            JudgmentId::new(format!("judgment:{id}")).map_err(invalid)?,
            proposition.clone(),
            spec,
        )
        .map_err(|e| Failure::new("invalid_receipt", e))?;
    let mut store = InMemoryBeliefStore::default();
    store
        .insert_authorized_evidence_batch(batch)
        .map_err(|e| Failure::new("store_failed", e))?;
    store
        .insert_semantic_judgment(judgment.clone())
        .map_err(|e| Failure::new("store_failed", e))?;
    let mut result = json!({
        "schema":"belief_semantic_result", "schemaVersion":1, "status":"ok", "id":id,
        "judgment": {"id":judgment.judgment().id().as_str(), "outcome":judgment.judgment().outcome().as_str(),
            "score":judgment.judgment().confidence().value(),"semantics":"conditional_option_probability"},
        "decision":crate::explain::receipt_report(judgment.provenance().decision()),
        "authorization":{"profile":policy.profile().as_str(),"inferenceClass":class.as_str()},
        "belief":null,"explanation":null,"request":original,
    });
    if judgment.judgment().outcome() != JudgmentOutcome::Unknown {
        let claim = Claim::from_judgment(
            ClaimId::new(format!("claim:{id}")).map_err(invalid)?,
            proposition,
            judgment.judgment().id().clone(),
        );
        let basis = JudgmentBasis::new(
            judgment.judgment().clone(),
            EvidenceFamilyId::new(format!("decision:{id}")).map_err(invalid)?,
            evidence
                .into_iter()
                .map(|e| EvidenceUse::new(e.evidence, e.purpose))
                .collect(),
        )
        .map_err(invalid)?;
        let inference = InferenceRequest::new(
            InferenceRunId::new(format!("run:{id}")).map_err(invalid)?,
            BeliefId::new(format!("belief:{id}")).map_err(invalid)?,
            rule,
            claim,
            vec![basis],
        )
        .map_err(invalid)?;
        let derived = derive_belief(&mut store, &policy, inference, |_| Ok(()))
            .map_err(|e| Failure::new("inference_failed", e))?;
        result["belief"] = json!({"id":derived.explanation.belief.value.id().as_str(),"value":derived.explanation.belief.value.value().value(),"semantics":"soft_truth"});
        result["explanation"] =
            serde_json::to_value(crate::explain::provenance_report(&derived.explanation))
                .map_err(invalid)?;
    }
    Ok(result)
}

pub struct FixtureEngine {
    pub scores: BTreeMap<String, f64>,
}
impl SemanticDecisionEngine for FixtureEngine {
    fn name(&self) -> &'static str {
        "fixture"
    }
    fn decide(
        &self,
        request: &AuthorizedDecisionRequest,
    ) -> Result<SemanticDecisionReceipt, DecisionEngineError> {
        SemanticDecisionReceipt::new(
            request.request(),
            "fixture",
            "v1",
            "scripted-scores",
            "v1",
            "deterministic-fixture",
            "fixture:no-model-prompt",
            "scripted-option-probabilities",
            self.scores.clone(),
        )
        .map_err(|e| DecisionEngineError::new(e.to_string()))
    }
}
