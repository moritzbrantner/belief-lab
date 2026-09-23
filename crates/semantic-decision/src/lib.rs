use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use belief_core::{
    EvidenceId, EvidencePurpose, EvidenceRef, InferenceClass, Judgment, JudgmentId,
    JudgmentOutcome, JudgmentSpecRef, ModelError, Proposition, Score, ScoreSemantics,
};
use belief_policy::{AuthorizationProfile, PolicyConfig};
use serde_json::Value;

pub const MAX_DECISION_OPTIONS: usize = 16;
pub const SUPPORTS_OPTION_ID: &str = "supports";
pub const CONTRADICTS_OPTION_ID: &str = "contradicts";
pub const UNKNOWN_OPTION_ID: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionOption {
    id: String,
    description: String,
}

impl DecisionOption {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, DecisionRequestError> {
        Ok(Self {
            id: required("option.id", id.into())?,
            description: required("option.description", description.into())?,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionEvidenceUse {
    pub evidence: EvidenceRef,
    pub purpose: EvidencePurpose,
}

impl DecisionEvidenceUse {
    pub fn new(evidence: EvidenceRef, purpose: EvidencePurpose) -> Self {
        Self { evidence, purpose }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRequest {
    id: String,
    state: Value,
    question: String,
    options: Vec<DecisionOption>,
    class: InferenceClass,
    evidence: Vec<DecisionEvidenceUse>,
}

impl DecisionRequest {
    pub fn new(
        id: impl Into<String>,
        state: Value,
        question: impl Into<String>,
        options: Vec<DecisionOption>,
        class: InferenceClass,
        evidence: Vec<DecisionEvidenceUse>,
    ) -> Result<Self, DecisionRequestError> {
        let id = required("decision.id", id.into())?;
        let question = required("decision.question", question.into())?;
        validate_state(&state)?;

        if !(2..=MAX_DECISION_OPTIONS).contains(&options.len()) {
            return Err(DecisionRequestError::InvalidOptionCount(options.len()));
        }

        let mut option_ids = BTreeSet::new();
        for option in &options {
            if !option_ids.insert(option.id.clone()) {
                return Err(DecisionRequestError::DuplicateOption(option.id.clone()));
            }
        }

        if evidence.is_empty() {
            return Err(DecisionRequestError::NoEvidence);
        }

        Ok(Self {
            id,
            state,
            question,
            options,
            class,
            evidence,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn state(&self) -> &Value {
        &self.state
    }

    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn options(&self) -> &[DecisionOption] {
        &self.options
    }

    pub fn class(&self) -> InferenceClass {
        self.class
    }

    pub fn evidence(&self) -> &[DecisionEvidenceUse] {
        &self.evidence
    }

    pub fn authorize(
        self,
        policy: &PolicyConfig,
    ) -> Result<AuthorizedDecisionRequest, DecisionAuthorizationError> {
        if !policy.allows_inference(self.class) {
            return Err(DecisionAuthorizationError::InferenceDenied(self.class));
        }

        let mut source_scopes = BTreeSet::new();
        let mut evidence_uses = BTreeSet::new();
        for evidence_use in &self.evidence {
            let evidence = &evidence_use.evidence;
            if !policy.allows_evidence_ref(evidence, evidence_use.purpose) {
                return Err(DecisionAuthorizationError::EvidenceDenied {
                    evidence: evidence.id.clone(),
                    class: evidence.class,
                    purpose: evidence_use.purpose,
                });
            }
            source_scopes.insert((
                evidence.provenance.source.repository.clone(),
                evidence.provenance.source.scope_id.clone(),
            ));
            evidence_uses.insert((evidence.id.clone(), evidence_use.purpose));
        }

        if source_scopes.len() > 1 && !policy.allow_cross_source_join {
            return Err(DecisionAuthorizationError::CrossSourceJoinDenied { source_scopes });
        }

        let authorization = DecisionAuthorizationReceipt {
            profile: policy.profile(),
            inference_class: self.class,
            source_scopes: source_scopes.clone(),
            evidence_uses,
            cross_source_join: source_scopes.len() > 1,
        };

        Ok(AuthorizedDecisionRequest {
            request: self,
            authorization,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionAuthorizationReceipt {
    profile: AuthorizationProfile,
    inference_class: InferenceClass,
    source_scopes: BTreeSet<(String, String)>,
    evidence_uses: BTreeSet<(EvidenceId, EvidencePurpose)>,
    cross_source_join: bool,
}

impl DecisionAuthorizationReceipt {
    pub fn profile(&self) -> AuthorizationProfile {
        self.profile
    }

    pub fn inference_class(&self) -> InferenceClass {
        self.inference_class
    }

    pub fn source_scopes(&self) -> &BTreeSet<(String, String)> {
        &self.source_scopes
    }

    pub fn evidence_uses(&self) -> &BTreeSet<(EvidenceId, EvidencePurpose)> {
        &self.evidence_uses
    }

    pub fn cross_source_join(&self) -> bool {
        self.cross_source_join
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorizedDecisionRequest {
    request: DecisionRequest,
    authorization: DecisionAuthorizationReceipt,
}

impl AuthorizedDecisionRequest {
    pub fn request(&self) -> &DecisionRequest {
        &self.request
    }

    pub fn authorization(&self) -> &DecisionAuthorizationReceipt {
        &self.authorization
    }

    pub fn into_three_way_judgment(
        self,
        receipt: SemanticDecisionReceipt,
        judgment_id: JudgmentId,
        proposition: Proposition,
        spec: JudgmentSpecRef,
    ) -> Result<SemanticJudgment, SemanticDecisionError> {
        if receipt.request != self.request {
            return Err(SemanticDecisionError::ReceiptRequestMismatch {
                request_id: self.request.id.clone(),
            });
        }

        let option_ids = self
            .request
            .options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<BTreeSet<_>>();
        let expected =
            BTreeSet::from([SUPPORTS_OPTION_ID, CONTRADICTS_OPTION_ID, UNKNOWN_OPTION_ID]);
        if option_ids != expected {
            return Err(SemanticDecisionError::NotThreeWaySupportDecision);
        }

        let outcome = match receipt.selected_option.as_str() {
            SUPPORTS_OPTION_ID => JudgmentOutcome::Supports,
            CONTRADICTS_OPTION_ID => JudgmentOutcome::Contradicts,
            UNKNOWN_OPTION_ID => JudgmentOutcome::Unknown,
            _ => return Err(SemanticDecisionError::NotThreeWaySupportDecision),
        };
        let confidence = *receipt
            .scores
            .get(&receipt.selected_option)
            .expect("validated receipt must score its selected option");
        let evidence = self
            .request
            .evidence
            .iter()
            .map(|item| item.evidence.id.clone())
            .collect::<BTreeSet<_>>();
        let model_revision = format!(
            "{}:{}@{}",
            receipt.provider, receipt.model, receipt.model_revision
        );
        let judgment = Judgment::new(
            judgment_id,
            proposition,
            outcome,
            Score::new(confidence, ScoreSemantics::ConditionalOptionProbability)
                .map_err(SemanticDecisionError::Model)?,
            evidence,
            spec,
            model_revision,
        )
        .map_err(SemanticDecisionError::Model)?;

        Ok(SemanticJudgment {
            judgment,
            provenance: SemanticJudgmentProvenance {
                authorization: self.authorization,
                decision: receipt,
            },
        })
    }
}

pub trait SemanticDecisionEngine {
    fn name(&self) -> &'static str;

    fn decide(
        &self,
        request: &AuthorizedDecisionRequest,
    ) -> Result<SemanticDecisionReceipt, DecisionEngineError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticDecisionReceipt {
    request: DecisionRequest,
    provider: String,
    provider_revision: String,
    model: String,
    model_revision: String,
    runtime: String,
    prompt_sha256: String,
    readout: String,
    scores: BTreeMap<String, f64>,
    selected_option: String,
}

impl SemanticDecisionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request: &DecisionRequest,
        provider: impl Into<String>,
        provider_revision: impl Into<String>,
        model: impl Into<String>,
        model_revision: impl Into<String>,
        runtime: impl Into<String>,
        prompt_sha256: impl Into<String>,
        readout: impl Into<String>,
        scores: BTreeMap<String, f64>,
    ) -> Result<Self, SemanticDecisionError> {
        let expected = request
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect::<BTreeSet<_>>();
        let actual = scores.keys().cloned().collect::<BTreeSet<_>>();
        if expected != actual {
            return Err(SemanticDecisionError::OptionScoresMismatch { expected, actual });
        }

        if scores
            .values()
            .any(|score| !score.is_finite() || !(0.0..=1.0).contains(score))
        {
            return Err(SemanticDecisionError::InvalidOptionScore);
        }

        let total = scores.values().sum::<f64>();
        if (total - 1.0).abs() > 1e-6 {
            return Err(SemanticDecisionError::ScoresNotNormalized(total));
        }

        let mut selected = request.options[0].id.clone();
        let mut selected_score = scores[&selected];
        for option in request.options.iter().skip(1) {
            let score = scores[&option.id];
            if score > selected_score {
                selected = option.id.clone();
                selected_score = score;
            }
        }

        Ok(Self {
            request: request.clone(),
            provider: semantic_required("decision.provider", provider.into())?,
            provider_revision: semantic_required(
                "decision.provider_revision",
                provider_revision.into(),
            )?,
            model: semantic_required("decision.model", model.into())?,
            model_revision: semantic_required("decision.model_revision", model_revision.into())?,
            runtime: semantic_required("decision.runtime", runtime.into())?,
            prompt_sha256: semantic_required("decision.prompt_sha256", prompt_sha256.into())?,
            readout: semantic_required("decision.readout", readout.into())?,
            scores,
            selected_option: selected,
        })
    }

    pub fn request_id(&self) -> &str {
        &self.request.id
    }

    pub fn request(&self) -> &DecisionRequest {
        &self.request
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn provider_revision(&self) -> &str {
        &self.provider_revision
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn model_revision(&self) -> &str {
        &self.model_revision
    }

    pub fn runtime(&self) -> &str {
        &self.runtime
    }

    pub fn prompt_sha256(&self) -> &str {
        &self.prompt_sha256
    }

    pub fn readout(&self) -> &str {
        &self.readout
    }

    pub fn scores(&self) -> &BTreeMap<String, f64> {
        &self.scores
    }

    pub fn selected_option(&self) -> &str {
        &self.selected_option
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticJudgmentProvenance {
    authorization: DecisionAuthorizationReceipt,
    decision: SemanticDecisionReceipt,
}

impl SemanticJudgmentProvenance {
    pub fn authorization(&self) -> &DecisionAuthorizationReceipt {
        &self.authorization
    }

    pub fn decision(&self) -> &SemanticDecisionReceipt {
        &self.decision
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticJudgment {
    judgment: Judgment,
    provenance: SemanticJudgmentProvenance,
}

impl SemanticJudgment {
    pub fn judgment(&self) -> &Judgment {
        &self.judgment
    }

    pub fn provenance(&self) -> &SemanticJudgmentProvenance {
        &self.provenance
    }

    pub fn into_parts(self) -> (Judgment, SemanticJudgmentProvenance) {
        (self.judgment, self.provenance)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionRequestError {
    EmptyField(&'static str),
    InvalidState,
    InvalidOptionCount(usize),
    DuplicateOption(String),
    NoEvidence,
}

impl fmt::Display for DecisionRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} cannot be empty"),
            Self::InvalidState => {
                f.write_str("decision state must be a non-empty string, object, or array")
            }
            Self::InvalidOptionCount(count) => write!(
                f,
                "semantic decisions require 2-{MAX_DECISION_OPTIONS} options, got {count}"
            ),
            Self::DuplicateOption(id) => {
                write!(f, "duplicate decision option id {id:?}")
            }
            Self::NoEvidence => {
                f.write_str("semantic decisions require at least one evidence input")
            }
        }
    }
}

impl std::error::Error for DecisionRequestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionAuthorizationError {
    InferenceDenied(InferenceClass),
    EvidenceDenied {
        evidence: EvidenceId,
        class: belief_core::EvidenceClass,
        purpose: EvidencePurpose,
    },
    CrossSourceJoinDenied {
        source_scopes: BTreeSet<(String, String)>,
    },
}

impl fmt::Display for DecisionAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InferenceDenied(class) => {
                write!(f, "inference class {} is not authorized", class.as_str())
            }
            Self::EvidenceDenied {
                evidence,
                class,
                purpose,
            } => write!(
                f,
                "evidence {evidence} ({}) is not authorized for {}",
                class.as_str(),
                purpose.as_str()
            ),
            Self::CrossSourceJoinDenied { source_scopes } => write!(
                f,
                "joining {} distinct source scopes is not authorized",
                source_scopes.len()
            ),
        }
    }
}

impl std::error::Error for DecisionAuthorizationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionEngineError {
    message: String,
}

impl DecisionEngineError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DecisionEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DecisionEngineError {}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticDecisionError {
    EmptyField(&'static str),
    OptionScoresMismatch {
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
    InvalidOptionScore,
    ScoresNotNormalized(f64),
    ReceiptRequestMismatch {
        request_id: String,
    },
    NotThreeWaySupportDecision,
    Model(ModelError),
}

impl fmt::Display for SemanticDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} cannot be empty"),
            Self::OptionScoresMismatch { expected, actual } => write!(
                f,
                "decision scores use option ids {actual:?}, expected {expected:?}"
            ),
            Self::InvalidOptionScore => {
                f.write_str(
                    "decision option scores must be finite and in [0, 1]",
                )
            }
            Self::ScoresNotNormalized(total) => {
                write!(
                    f,
                    "decision option scores must sum to 1, got {total}"
                )
            }
            Self::ReceiptRequestMismatch { request_id } => write!(
                f,
                "decision receipt does not match the full authorized request {request_id:?}"
            ),
            Self::NotThreeWaySupportDecision => write!(
                f,
                "judgment conversion requires exactly {SUPPORTS_OPTION_ID:?}, {CONTRADICTS_OPTION_ID:?}, and {UNKNOWN_OPTION_ID:?} options"
            ),
            Self::Model(error) => {
                write!(
                    f,
                    "could not construct semantic judgment: {error}"
                )
            }
        }
    }
}

impl std::error::Error for SemanticDecisionError {}

fn required(field: &'static str, value: String) -> Result<String, DecisionRequestError> {
    if value.trim().is_empty() {
        Err(DecisionRequestError::EmptyField(field))
    } else {
        Ok(value)
    }
}

fn semantic_required(field: &'static str, value: String) -> Result<String, SemanticDecisionError> {
    if value.trim().is_empty() {
        Err(SemanticDecisionError::EmptyField(field))
    } else {
        Ok(value)
    }
}

fn validate_state(state: &Value) -> Result<(), DecisionRequestError> {
    let valid = match state {
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(DecisionRequestError::InvalidState)
    }
}

#[cfg(test)]
mod tests {
    use belief_core::{EntityId, EvidenceClass, EvidenceFamilyId, ProducerRef, SourceRef};

    use super::*;

    fn evidence(id: &str, scope: &str, class: EvidenceClass) -> EvidenceRef {
        EvidenceRef::new(
            EvidenceId::new(id).unwrap(),
            Some(EntityId::new("person:alice").unwrap()),
            class,
            EvidenceFamilyId::new(format!("family:{id}")).unwrap(),
            None,
            belief_core::Provenance::new(
                SourceRef::new(
                    "youtube-corpus",
                    scope,
                    format!("{scope}#{id}"),
                    "sha256:source",
                )
                .unwrap(),
                ProducerRef::new("fixture", "commit:1", None, None).unwrap(),
                [],
            ),
        )
    }

    fn options() -> Vec<DecisionOption> {
        [
            (SUPPORTS_OPTION_ID, "The evidence supports the proposition."),
            (
                CONTRADICTS_OPTION_ID,
                "The evidence contradicts the proposition.",
            ),
            (UNKNOWN_OPTION_ID, "The evidence is insufficient."),
        ]
        .into_iter()
        .map(|(id, description)| DecisionOption::new(id, description).unwrap())
        .collect()
    }

    fn request(evidence: Vec<DecisionEvidenceUse>) -> DecisionRequest {
        DecisionRequest::new(
            "decision:1",
            Value::String("bounded evidence text".into()),
            "Does the evidence support the proposition?",
            options(),
            InferenceClass::Descriptive,
            evidence,
        )
        .unwrap()
    }

    fn policy() -> PolicyConfig {
        PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap()
    }

    #[test]
    fn request_rejects_duplicate_options() {
        let evidence = evidence("transcript:1", "video:1", EvidenceClass::Transcript);
        let result = DecisionRequest::new(
            "decision:1",
            Value::String("state".into()),
            "question",
            vec![
                DecisionOption::new("same", "first").unwrap(),
                DecisionOption::new("same", "second").unwrap(),
            ],
            InferenceClass::Descriptive,
            vec![DecisionEvidenceUse::new(
                evidence,
                EvidencePurpose::Corroboration,
            )],
        );

        assert!(matches!(
            result,
            Err(DecisionRequestError::DuplicateOption(_))
        ));
    }

    #[test]
    fn policy_denies_semantic_decision_over_unlisted_evidence() {
        let face = evidence("face:1", "video:1", EvidenceClass::FaceTrackReference);
        let request = request(vec![DecisionEvidenceUse::new(
            face,
            EvidencePurpose::Corroboration,
        )]);

        assert!(matches!(
            request.authorize(&policy()),
            Err(DecisionAuthorizationError::EvidenceDenied { .. })
        ));
    }

    #[test]
    fn cross_source_semantic_decision_requires_opt_in() {
        let first = evidence("transcript:1", "video:1", EvidenceClass::Transcript);
        let second = evidence("transcript:2", "video:2", EvidenceClass::Transcript);
        let request = request(vec![
            DecisionEvidenceUse::new(first, EvidencePurpose::Corroboration),
            DecisionEvidenceUse::new(second, EvidencePurpose::Corroboration),
        ]);

        assert!(matches!(
            request.authorize(&policy()),
            Err(DecisionAuthorizationError::CrossSourceJoinDenied { .. })
        ));
    }

    #[test]
    fn receipt_cannot_be_reused_for_a_different_request_with_the_same_id() {
        let evidence = evidence("transcript:1", "video:1", EvidenceClass::Transcript);
        let authorized = request(vec![DecisionEvidenceUse::new(
            evidence.clone(),
            EvidencePurpose::Corroboration,
        )])
        .authorize(&policy())
        .unwrap();
        let receipt = SemanticDecisionReceipt::new(
            authorized.request(),
            "semif",
            "git:semif-1",
            "Qwen/Qwen3.5-4B",
            "model:1",
            "llamacpp",
            "sha256:prompt",
            "native-option-logits",
            BTreeMap::from([
                (SUPPORTS_OPTION_ID.into(), 0.7),
                (CONTRADICTS_OPTION_ID.into(), 0.2),
                (UNKNOWN_OPTION_ID.into(), 0.1),
            ]),
        )
        .unwrap();

        let different = DecisionRequest::new(
            "decision:1",
            Value::String("different bounded evidence text".into()),
            "Does the evidence support the proposition?",
            options(),
            InferenceClass::Descriptive,
            vec![DecisionEvidenceUse::new(
                evidence,
                EvidencePurpose::Corroboration,
            )],
        )
        .unwrap()
        .authorize(&policy())
        .unwrap();

        assert!(matches!(
            different.into_three_way_judgment(
                receipt,
                JudgmentId::new("judgment:1").unwrap(),
                Proposition::new(
                    EntityId::new("person:alice").unwrap(),
                    belief_core::Predicate::new("uses").unwrap(),
                    belief_core::ObjectValue::text("Linux").unwrap(),
                ),
                JudgmentSpecRef::new("support-check", "v1").unwrap(),
            ),
            Err(SemanticDecisionError::ReceiptRequestMismatch { .. })
        ));
    }

    #[test]
    fn three_way_receipt_becomes_conditional_probability_judgment() {
        let evidence = evidence("transcript:1", "video:1", EvidenceClass::Transcript);
        let authorized = request(vec![DecisionEvidenceUse::new(
            evidence,
            EvidencePurpose::Corroboration,
        )])
        .authorize(&policy())
        .unwrap();
        let receipt = SemanticDecisionReceipt::new(
            authorized.request(),
            "semif",
            "git:semif-1",
            "Qwen/Qwen3.5-4B",
            "model:1",
            "llamacpp",
            "sha256:prompt",
            "native-option-logits",
            BTreeMap::from([
                (SUPPORTS_OPTION_ID.into(), 0.7),
                (CONTRADICTS_OPTION_ID.into(), 0.2),
                (UNKNOWN_OPTION_ID.into(), 0.1),
            ]),
        )
        .unwrap();

        let semantic = authorized
            .into_three_way_judgment(
                receipt,
                JudgmentId::new("judgment:1").unwrap(),
                Proposition::new(
                    EntityId::new("person:alice").unwrap(),
                    belief_core::Predicate::new("uses").unwrap(),
                    belief_core::ObjectValue::text("Linux").unwrap(),
                ),
                JudgmentSpecRef::new("support-check", "v1").unwrap(),
            )
            .unwrap();

        assert_eq!(semantic.judgment().outcome, JudgmentOutcome::Supports);
        assert_eq!(
            semantic.judgment().confidence.semantics(),
            ScoreSemantics::ConditionalOptionProbability
        );
        assert_eq!(
            semantic.provenance().decision().selected_option(),
            "supports"
        );
    }
}
