//! `belief explain`: run an evidence-interchange batch (plus an optional deterministic judgment
//! script) through policy, inference, and the store, and report what happened.
//!
//! The report has no timestamps or absolute paths, and every list has a defined order, so the
//! output can be compared byte-for-byte against checked-in expectations.

use std::collections::BTreeMap;
use std::error::Error;

use belief_core::{
    BeliefId, Claim, ClaimId, ClaimOrigin, EvidenceRef, InferenceRunId, ObjectValue, Proposition,
    Score, ScoreSemantics,
};
use belief_policy::PolicyConfig;
use belief_store::{BeliefExplanation, InMemoryBeliefStore, Stored, Validity};
use evidence_interchange::{
    EvidenceAdmission, ValidatedEvidenceBatch, EVIDENCE_INTERCHANGE_SCHEMA,
    EVIDENCE_INTERCHANGE_VERSION_V1,
};
use inference_core::{EvidenceUse, InferenceRequest, JudgmentBasis, TrustedInferenceRule};
use serde::Serialize;

use crate::judgments::{JudgmentScript, PlannedInference};
use crate::pipeline::{derive_belief, DeriveError};

pub const REPORT_KIND: &str = "belief_explain";
pub const REPORT_VERSION: u32 = 1;

/// Inputs for one explanation run. `policy` holds `BELIEF_*` pairs; nothing is read from the
/// process environment.
pub struct ExplainInput<'a> {
    pub evidence_json: &'a str,
    pub judgments_json: Option<&'a str>,
    pub policy: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub report: &'static str,
    pub report_version: u32,
    pub outcome: Outcome,
    pub policy: PolicyReport,
    pub rejection: Option<Rejection>,
    pub batch: Option<BatchReport>,
    pub inferences: Vec<InferenceReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every input was valid; policy decisions (including refusals) are in the report.
    Explained,
    /// An input was malformed or unsupported; the run failed closed before any import.
    InputRejected,
}

#[derive(Debug, Serialize)]
pub struct PolicyReport {
    pub profile: String,
    /// Explicit settings supplied for this run; everything else is the profile default.
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct Rejection {
    pub stage: RejectionStage,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionStage {
    PolicyConfig,
    Interchange,
    JudgmentScript,
}

impl RejectionStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PolicyConfig => "policy configuration",
            Self::Interchange => "evidence interchange",
            Self::JudgmentScript => "judgment script",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BatchReport {
    pub schema: String,
    pub exporter: ExporterReport,
    pub revision: String,
    /// Evidence in topological (parents-first) order, as the importer returns it.
    pub evidence: Vec<EvidenceReport>,
    pub import: ImportReport,
}

#[derive(Debug, Serialize)]
pub struct ExporterReport {
    pub name: String,
    pub revision: String,
}

#[derive(Debug, Serialize)]
pub struct EvidenceReport {
    pub id: String,
    pub class: String,
    pub family: String,
    pub subject: Option<String>,
    pub score: Option<ScoreReport>,
    pub source: SourceReport,
    pub producer: ProducerReport,
    pub parents: Vec<String>,
    pub admission: AdmissionReport,
}

#[derive(Debug, Serialize)]
pub struct AdmissionReport {
    pub admitted: bool,
    pub admitted_purposes: Vec<String>,
    /// Denial reason per purpose that was not admitted.
    pub denied_purposes: BTreeMap<String, String>,
    /// Distinct denial reasons when the record is rejected outright.
    pub rejection_reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SourceReport {
    pub repository: String,
    pub scope_id: String,
    pub record_id: String,
    pub revision: String,
}

#[derive(Debug, Serialize)]
pub struct ProducerReport {
    pub name: String,
    pub revision: String,
    pub model: Option<String>,
    pub config_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScoreReport {
    pub value: f64,
    pub semantics: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub imported: bool,
    pub stored: usize,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InferenceReport {
    pub id: String,
    pub class: &'static str,
    pub proposition: PropositionReport,
    pub status: InferenceStatus,
    pub reason: Option<String>,
    pub rule: Option<&'static str>,
    pub judgments: Vec<JudgmentReport>,
    pub authorization: Option<AuthorizationReport>,
    pub belief: Option<BeliefReport>,
    pub provenance: Option<ProvenanceReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceStatus {
    /// A belief was authorized, inferred, stored, and explained.
    Derived,
    /// Policy (or the absence of a trusted rule) refused the inference; nothing was stored.
    Refused,
    /// The evidence batch was not imported, so the inference could not run.
    NotRun,
    /// Authorized, but the engine found no usable signal; nothing was stored.
    NoBelief,
}

impl InferenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
            Self::Refused => "refused",
            Self::NotRun => "not run",
            Self::NoBelief => "no belief",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PropositionReport {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Serialize)]
pub struct JudgmentReport {
    pub id: String,
    pub outcome: &'static str,
    pub confidence: ScoreReport,
    pub correlation_group: String,
    pub evidence: Vec<EvidenceUseReport>,
    pub spec: String,
    pub model_revision: String,
    /// How the baseline engine used this judgment; `null` when inference did not run.
    pub selection: Option<Selection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    Selected,
    IgnoredCorrelated,
    IgnoredUnknown,
}

impl Selection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::IgnoredCorrelated => "ignored: weaker judgment in the same correlation group",
            Self::IgnoredUnknown => "ignored: unknown outcome carries no signal",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EvidenceUseReport {
    pub evidence: String,
    pub purpose: &'static str,
}

#[derive(Debug, Serialize)]
pub struct AuthorizationReport {
    pub profile: &'static str,
    pub inference_class: &'static str,
    pub source_scopes: Vec<String>,
    pub cross_source_join: bool,
    pub evidence_uses: Vec<EvidenceUseReport>,
}

#[derive(Debug, Serialize)]
pub struct BeliefReport {
    pub id: String,
    pub claim: String,
    pub value: f64,
    pub semantics: &'static str,
    pub meaning: &'static str,
}

/// The chain read back from the store: belief → derivation → claim → judgments → evidence.
#[derive(Debug, Serialize)]
pub struct ProvenanceReport {
    pub belief: String,
    pub validity: String,
    pub derivation: DerivationReport,
    pub claim: ClaimReport,
    pub judgments: Vec<StoredJudgmentReport>,
    pub evidence: Vec<StoredEvidenceReport>,
}

#[derive(Debug, Serialize)]
pub struct DerivationReport {
    pub rule: String,
    pub inference_run: String,
    pub judgments: Vec<String>,
    pub evidence: Vec<String>,
    pub claims: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ClaimReport {
    pub id: String,
    pub validity: String,
    pub origin: String,
}

#[derive(Debug, Serialize)]
pub struct StoredJudgmentReport {
    pub id: String,
    pub validity: String,
    pub outcome: &'static str,
    pub confidence: ScoreReport,
    pub spec: String,
    pub model_revision: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct StoredEvidenceReport {
    pub id: String,
    pub validity: String,
    pub class: &'static str,
    pub source: SourceReport,
    pub producer: ProducerReport,
    pub parents: Vec<String>,
}

/// Runs the full offline path. Returns `Err` only for internal inconsistencies (for example a
/// store write rejected after authorization); input and policy outcomes are in the report.
pub fn explain(input: &ExplainInput<'_>) -> Result<Report, Box<dyn Error>> {
    let requested_profile = input
        .policy
        .get("BELIEF_POLICY_PROFILE")
        .cloned()
        .unwrap_or_else(|| "observe_only".into());
    let mut report = Report {
        report: REPORT_KIND,
        report_version: REPORT_VERSION,
        outcome: Outcome::Explained,
        policy: PolicyReport {
            profile: requested_profile,
            settings: input.policy.clone(),
        },
        rejection: None,
        batch: None,
        inferences: Vec::new(),
    };

    let policy = match PolicyConfig::from_pairs(input.policy.clone()) {
        Ok(policy) => policy,
        Err(error) => return Ok(report.reject(RejectionStage::PolicyConfig, error)),
    };
    report.policy.profile = policy.profile().as_str().into();

    let batch = match ValidatedEvidenceBatch::parse_json(input.evidence_json) {
        Ok(batch) => batch,
        Err(error) => return Ok(report.reject(RejectionStage::Interchange, error)),
    };
    let script = match input.judgments_json.map(JudgmentScript::parse_json) {
        None => JudgmentScript {
            inferences: Vec::new(),
        },
        Some(Ok(script)) => script,
        Some(Err(error)) => return Ok(report.reject(RejectionStage::JudgmentScript, error)),
    };
    if let Some(missing) = script
        .referenced_evidence()
        .into_iter()
        .find(|id| !batch.evidence().iter().any(|evidence| &evidence.id == *id))
    {
        return Ok(report.reject(
            RejectionStage::JudgmentScript,
            format!("judgments reference evidence {missing}, which is not in the batch"),
        ));
    }

    let admissions = batch.admission(&policy);
    let mut batch_report = BatchReport {
        schema: format!("{EVIDENCE_INTERCHANGE_SCHEMA}@{EVIDENCE_INTERCHANGE_VERSION_V1}"),
        exporter: ExporterReport {
            name: batch.exporter_name().into(),
            revision: batch.exporter_revision().into(),
        },
        revision: batch.revision().into(),
        evidence: batch
            .evidence()
            .iter()
            .zip(&admissions)
            .map(|(evidence, admission)| evidence_report(evidence, admission))
            .collect(),
        import: ImportReport {
            imported: false,
            stored: 0,
            reason: None,
        },
    };

    let mut store = InMemoryBeliefStore::default();
    match batch.authorize(&policy) {
        Ok(authorized) => {
            let outcome = store.insert_authorized_evidence_batch(authorized)?;
            batch_report.import.imported = true;
            batch_report.import.stored = outcome.inserted;
        }
        Err(error) => {
            let rejected = admissions.iter().filter(|a| !a.is_admitted()).count();
            batch_report.import.reason = Some(format!(
                "{error}; import is all-or-nothing, so {rejected} rejected of {} refuses the whole batch",
                admissions.len()
            ));
        }
    }
    let imported = batch_report.import.imported;
    report.batch = Some(batch_report);

    for planned in script.inferences {
        report
            .inferences
            .push(run_inference(&mut store, &policy, imported, planned)?);
    }

    Ok(report)
}

impl Report {
    fn reject(mut self, stage: RejectionStage, reason: impl ToString) -> Self {
        self.outcome = Outcome::InputRejected;
        self.rejection = Some(Rejection {
            stage,
            reason: reason.to_string(),
        });
        self
    }
}

fn run_inference(
    store: &mut InMemoryBeliefStore,
    policy: &PolicyConfig,
    imported: bool,
    planned: PlannedInference,
) -> Result<InferenceReport, Box<dyn Error>> {
    let mut report = InferenceReport {
        id: planned.id.clone(),
        class: planned.class.as_str(),
        proposition: proposition_report(&planned.proposition),
        status: InferenceStatus::Refused,
        reason: None,
        rule: None,
        judgments: planned
            .judgments
            .iter()
            .map(|planned| JudgmentReport {
                id: planned.judgment.id().to_string(),
                outcome: planned.judgment.outcome().as_str(),
                confidence: score_report(planned.judgment.confidence()),
                correlation_group: planned.correlation_group.to_string(),
                evidence: planned
                    .evidence
                    .iter()
                    .map(|(evidence, purpose)| EvidenceUseReport {
                        evidence: evidence.to_string(),
                        purpose: purpose.as_str(),
                    })
                    .collect(),
                spec: format!(
                    "{}@{}",
                    planned.judgment.spec().name,
                    planned.judgment.spec().revision
                ),
                model_revision: planned.judgment.model_revision().into(),
                selection: None,
            })
            .collect(),
        authorization: None,
        belief: None,
        provenance: None,
    };

    if !imported {
        report.status = InferenceStatus::NotRun;
        report.reason = Some("the evidence batch was not imported".into());
        return Ok(report);
    }
    if let Err(denial) = policy.inference_decision(planned.class) {
        report.reason = Some(denial.to_string());
        return Ok(report);
    }
    let Some(rule) = TrustedInferenceRule::for_class(planned.class) else {
        report.reason = Some(format!(
            "no trusted inference rule implements {}",
            planned.class.as_str()
        ));
        return Ok(report);
    };
    report.rule = Some(rule.rule_id());

    let mut bases = Vec::with_capacity(planned.judgments.len());
    for judgment in &planned.judgments {
        let evidence = judgment
            .evidence
            .iter()
            .map(|(id, purpose)| {
                let stored = store
                    .evidence(id)
                    .ok_or_else(|| format!("imported evidence {id} is missing from the store"))?;
                Ok(EvidenceUse::new(stored.value.clone(), *purpose))
            })
            .collect::<Result<Vec<_>, String>>()?;
        bases.push(JudgmentBasis::new(
            judgment.judgment.clone(),
            judgment.correlation_group.clone(),
            evidence,
        )?);
    }

    let origin = planned
        .judgments
        .first()
        .map(|judgment| judgment.judgment.id().clone())
        .ok_or("validated scripts always have at least one judgment")?;
    let claim = Claim::from_judgment(
        ClaimId::new(format!("claim:{}", planned.id))?,
        planned.proposition,
        origin,
    );
    let request = InferenceRequest::new(
        InferenceRunId::new(format!("run:{}", planned.id))?,
        BeliefId::new(format!("belief:{}", planned.id))?,
        rule,
        claim,
        bases,
    )?;
    let judgments = planned.judgments;
    let record = |store: &mut InMemoryBeliefStore| {
        judgments
            .into_iter()
            .try_for_each(|planned| store.insert_judgment(planned.judgment))
    };

    match derive_belief(store, policy, request, record) {
        Ok(derived) => {
            for judgment in &mut report.judgments {
                judgment.selection = Some(
                    if derived
                        .selected_judgments
                        .iter()
                        .any(|id| id.as_str() == judgment.id)
                    {
                        Selection::Selected
                    } else if derived
                        .ignored_correlated_judgments
                        .iter()
                        .any(|id| id.as_str() == judgment.id)
                    {
                        Selection::IgnoredCorrelated
                    } else {
                        Selection::IgnoredUnknown
                    },
                );
            }
            let explanation = &derived.explanation;
            report.status = InferenceStatus::Derived;
            report.authorization = Some(authorization_report(explanation));
            let value = explanation.belief.value.value();
            report.belief = Some(BeliefReport {
                id: explanation.belief.value.id().to_string(),
                claim: explanation.belief.value.claim().to_string(),
                value: value.value(),
                semantics: value.semantics().as_str(),
                meaning: semantics_meaning(value.semantics()),
            });
            report.provenance = Some(provenance_report(explanation));
        }
        Err(DeriveError::Unauthorized(error)) => report.reason = Some(error.to_string()),
        Err(DeriveError::NoBelief(error)) => {
            report.status = InferenceStatus::NoBelief;
            report.reason = Some(error.to_string());
        }
        Err(error @ DeriveError::Store(_)) => return Err(error.into()),
    }
    Ok(report)
}

/// What a score of these semantics may and may not be read as.
pub fn semantics_meaning(semantics: ScoreSemantics) -> &'static str {
    match semantics {
        ScoreSemantics::DetectorConfidence => "producer detector confidence; not a belief",
        ScoreSemantics::ModelConfidence => {
            "judgment model confidence; a heuristic signal, not a likelihood"
        }
        ScoreSemantics::ConditionalOptionProbability => {
            "provider option probability conditional on the offered options; not a belief"
        }
        ScoreSemantics::Similarity => "similarity; not a probability",
        ScoreSemantics::SoftTruth => {
            "heuristic soft truth in [0, 1] with 0.5 neutral; not a calibrated posterior probability"
        }
        ScoreSemantics::PosteriorProbability => "posterior probability from an inference engine",
    }
}

fn evidence_report(evidence: &EvidenceRef, admission: &EvidenceAdmission) -> EvidenceReport {
    EvidenceReport {
        id: evidence.id.to_string(),
        class: evidence.class.as_str().into(),
        family: evidence.family.to_string(),
        subject: evidence.subject.as_ref().map(ToString::to_string),
        score: evidence.score.map(score_report),
        source: source_report(evidence),
        producer: producer_report(evidence),
        parents: evidence
            .provenance
            .parent_evidence
            .iter()
            .map(ToString::to_string)
            .collect(),
        admission: AdmissionReport {
            admitted: admission.is_admitted(),
            admitted_purposes: admission
                .admitted_purposes
                .iter()
                .map(|purpose| purpose.as_str().to_string())
                .collect(),
            denied_purposes: admission
                .denied_purposes
                .iter()
                .map(|(purpose, denial)| (purpose.as_str().to_string(), denial.to_string()))
                .collect(),
            rejection_reasons: if admission.is_admitted() {
                Vec::new()
            } else {
                admission
                    .denial_reasons()
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            },
        },
    }
}

fn source_report(evidence: &EvidenceRef) -> SourceReport {
    let source = &evidence.provenance.source;
    SourceReport {
        repository: source.repository().into(),
        scope_id: source.scope_id().into(),
        record_id: source.record_id().into(),
        revision: source.revision().into(),
    }
}

fn producer_report(evidence: &EvidenceRef) -> ProducerReport {
    let producer = &evidence.provenance.producer;
    ProducerReport {
        name: producer.name().into(),
        revision: producer.revision().into(),
        model: producer.model().map(Into::into),
        config_hash: producer.config_hash().map(Into::into),
    }
}

fn score_report(score: Score) -> ScoreReport {
    ScoreReport {
        value: score.value(),
        semantics: score.semantics().as_str(),
    }
}

fn proposition_report(proposition: &Proposition) -> PropositionReport {
    PropositionReport {
        subject: proposition.subject.to_string(),
        predicate: proposition.predicate.as_str().into(),
        object: match &proposition.object {
            ObjectValue::Entity(entity) => entity.to_string(),
            ObjectValue::Text(text) => format!("{text:?}"),
            ObjectValue::Boolean(value) => value.to_string(),
        },
    }
}

fn authorization_report(explanation: &BeliefExplanation) -> AuthorizationReport {
    let receipt = &explanation.authorization;
    AuthorizationReport {
        profile: receipt.profile().as_str(),
        inference_class: receipt.inference_class().as_str(),
        source_scopes: receipt
            .source_scopes()
            .iter()
            .map(|(repository, scope)| format!("{repository}/{scope}"))
            .collect(),
        cross_source_join: receipt.cross_source_join(),
        evidence_uses: receipt
            .evidence_uses()
            .iter()
            .map(|(evidence, purpose)| EvidenceUseReport {
                evidence: evidence.to_string(),
                purpose: purpose.as_str(),
            })
            .collect(),
    }
}

fn provenance_report(explanation: &BeliefExplanation) -> ProvenanceReport {
    let derivation = &explanation.derivation;
    ProvenanceReport {
        belief: explanation.belief.value.id().to_string(),
        validity: validity(&explanation.belief),
        derivation: DerivationReport {
            rule: derivation.rule_id().into(),
            inference_run: derivation.inference_run().to_string(),
            judgments: derivation
                .judgments()
                .iter()
                .map(ToString::to_string)
                .collect(),
            evidence: derivation
                .evidence()
                .iter()
                .map(ToString::to_string)
                .collect(),
            claims: derivation
                .claims()
                .iter()
                .map(ToString::to_string)
                .collect(),
        },
        claim: ClaimReport {
            id: explanation.claim.value.id.to_string(),
            validity: validity(&explanation.claim),
            origin: match &explanation.claim.value.origin {
                ClaimOrigin::UserAssertion(evidence) => format!("user assertion {evidence}"),
                ClaimOrigin::Judgment(judgment) => format!("judgment {judgment}"),
                ClaimOrigin::Rule {
                    rule_id,
                    input_claims,
                } => format!(
                    "rule {rule_id} over {}",
                    input_claims
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            },
        },
        judgments: explanation
            .judgments
            .iter()
            .map(|stored| {
                let judgment = &stored.value;
                StoredJudgmentReport {
                    id: judgment.id().to_string(),
                    validity: validity(stored),
                    outcome: judgment.outcome().as_str(),
                    confidence: score_report(judgment.confidence()),
                    spec: format!("{}@{}", judgment.spec().name, judgment.spec().revision),
                    model_revision: judgment.model_revision().into(),
                    evidence: judgment
                        .evidence()
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                }
            })
            .collect(),
        evidence: explanation
            .evidence
            .iter()
            .map(|stored| {
                let evidence = &stored.value;
                StoredEvidenceReport {
                    id: evidence.id.to_string(),
                    validity: validity(stored),
                    class: evidence.class.as_str(),
                    source: source_report(evidence),
                    producer: producer_report(evidence),
                    parents: evidence
                        .provenance
                        .parent_evidence
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                }
            })
            .collect(),
    }
}

fn validity<T>(stored: &Stored<T>) -> String {
    match &stored.validity {
        Validity::Active => "active".into(),
        Validity::Invalidated { reason } => format!("invalidated: {reason}"),
    }
}
