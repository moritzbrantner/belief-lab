//! Deterministic judgment scripts for offline explanation runs.
//!
//! A script stands in for a judgment provider (such as SemIf) without running a model: it
//! records which propositions to assess, which judgments a provider returned, and which admitted
//! evidence each judgment used for which purpose. Policy, inference, and the store still run for
//! real on top of it.

use std::collections::BTreeSet;
use std::fmt;

use belief_core::{
    EntityId, EvidenceFamilyId, EvidenceId, EvidencePurpose, InferenceClass, Judgment, JudgmentId,
    JudgmentOutcome, JudgmentSpecRef, ModelError, ObjectValue, Predicate, Proposition, Score,
    ScoreSemantics,
};
use serde::Deserialize;

pub const JUDGMENT_SCRIPT_SCHEMA: &str = "belief_judgment_script";
pub const JUDGMENT_SCRIPT_VERSION_V1: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScriptV1 {
    schema: String,
    schema_version: u32,
    judgment_spec: SpecV1,
    model_revision: String,
    inferences: Vec<InferenceV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpecV1 {
    name: String,
    revision: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InferenceV1 {
    id: String,
    class: String,
    proposition: PropositionV1,
    judgments: Vec<JudgmentV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PropositionV1 {
    subject: String,
    predicate: String,
    object: ObjectV1,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ObjectV1 {
    Entity(String),
    Text(String),
    Boolean(bool),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JudgmentV1 {
    id: String,
    outcome: OutcomeV1,
    model_confidence: f64,
    correlation_group: String,
    evidence: Vec<EvidenceUseV1>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OutcomeV1 {
    Supports,
    Contradicts,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceUseV1 {
    id: String,
    purpose: String,
}

/// A validated script: every identifier, class, purpose, and score has been checked.
#[derive(Debug, Clone, PartialEq)]
pub struct JudgmentScript {
    pub inferences: Vec<PlannedInference>,
}

/// One requested inference and the judgments that would feed it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedInference {
    pub id: String,
    pub class: InferenceClass,
    pub proposition: Proposition,
    pub judgments: Vec<PlannedJudgment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedJudgment {
    pub judgment: Judgment,
    pub correlation_group: EvidenceFamilyId,
    /// Evidence uses in script order; ids match `judgment.evidence()`.
    pub evidence: Vec<(EvidenceId, EvidencePurpose)>,
}

impl JudgmentScript {
    pub fn parse_json(json: &str) -> Result<Self, ScriptError> {
        let script = serde_json::from_str::<ScriptV1>(json)
            .map_err(|error| ScriptError::InvalidJson(error.to_string()))?;
        if script.schema != JUDGMENT_SCRIPT_SCHEMA
            || script.schema_version != JUDGMENT_SCRIPT_VERSION_V1
        {
            return Err(ScriptError::UnsupportedSchema {
                schema: script.schema,
                version: script.schema_version,
            });
        }

        let spec = JudgmentSpecRef::new(script.judgment_spec.name, script.judgment_spec.revision)?;
        let mut inference_ids = BTreeSet::new();
        let mut judgment_ids = BTreeSet::new();
        let mut inferences = Vec::with_capacity(script.inferences.len());

        for inference in script.inferences {
            if inference.id.trim().is_empty() {
                return Err(ScriptError::Model(ModelError::EmptyField("inference.id")));
            }
            if !inference_ids.insert(inference.id.clone()) {
                return Err(ScriptError::DuplicateInference(inference.id));
            }
            let class = InferenceClass::parse(&inference.class).ok_or_else(|| {
                ScriptError::UnknownInferenceClass {
                    inference: inference.id.clone(),
                    class: inference.class.clone(),
                }
            })?;
            if inference.judgments.is_empty() {
                return Err(ScriptError::NoJudgments(inference.id));
            }

            let proposition = Proposition::new(
                EntityId::new(inference.proposition.subject)?,
                Predicate::new(inference.proposition.predicate)?,
                match inference.proposition.object {
                    ObjectV1::Entity(value) => ObjectValue::Entity(EntityId::new(value)?),
                    ObjectV1::Text(value) => ObjectValue::text(value)?,
                    ObjectV1::Boolean(value) => ObjectValue::Boolean(value),
                },
            );

            let mut judgments = Vec::with_capacity(inference.judgments.len());
            for judgment in inference.judgments {
                let id = JudgmentId::new(judgment.id)?;
                if !judgment_ids.insert(id.clone()) {
                    return Err(ScriptError::DuplicateJudgment(id));
                }
                let mut evidence = Vec::with_capacity(judgment.evidence.len());
                let mut seen = BTreeSet::new();
                for item in judgment.evidence {
                    let evidence_id = EvidenceId::new(item.id)?;
                    let purpose = EvidencePurpose::parse(&item.purpose).ok_or_else(|| {
                        ScriptError::UnknownPurpose {
                            judgment: id.clone(),
                            purpose: item.purpose.clone(),
                        }
                    })?;
                    if !seen.insert(evidence_id.clone()) {
                        return Err(ScriptError::DuplicateEvidenceUse {
                            judgment: id.clone(),
                            evidence: evidence_id,
                        });
                    }
                    evidence.push((evidence_id, purpose));
                }
                let outcome = match judgment.outcome {
                    OutcomeV1::Supports => JudgmentOutcome::Supports,
                    OutcomeV1::Contradicts => JudgmentOutcome::Contradicts,
                    OutcomeV1::Unknown => JudgmentOutcome::Unknown,
                };
                let value = Judgment::new(
                    id,
                    proposition.clone(),
                    outcome,
                    Score::new(judgment.model_confidence, ScoreSemantics::ModelConfidence)?,
                    evidence.iter().map(|(id, _)| id.clone()),
                    spec.clone(),
                    script.model_revision.clone(),
                )?;
                judgments.push(PlannedJudgment {
                    judgment: value,
                    correlation_group: EvidenceFamilyId::new(judgment.correlation_group)?,
                    evidence,
                });
            }

            inferences.push(PlannedInference {
                id: inference.id,
                class,
                proposition,
                judgments,
            });
        }

        Ok(Self { inferences })
    }

    /// Evidence ids referenced by any judgment, for checking against the evidence batch.
    pub fn referenced_evidence(&self) -> BTreeSet<&EvidenceId> {
        self.inferences
            .iter()
            .flat_map(|inference| &inference.judgments)
            .flat_map(|judgment| judgment.evidence.iter().map(|(id, _)| id))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptError {
    InvalidJson(String),
    UnsupportedSchema {
        schema: String,
        version: u32,
    },
    DuplicateInference(String),
    DuplicateJudgment(JudgmentId),
    DuplicateEvidenceUse {
        judgment: JudgmentId,
        evidence: EvidenceId,
    },
    NoJudgments(String),
    UnknownInferenceClass {
        inference: String,
        class: String,
    },
    UnknownPurpose {
        judgment: JudgmentId,
        purpose: String,
    },
    Model(ModelError),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid judgment script JSON: {error}"),
            Self::UnsupportedSchema { schema, version } => {
                write!(f, "unsupported judgment script {schema}@{version}")
            }
            Self::DuplicateInference(id) => write!(f, "duplicate inference id {id}"),
            Self::DuplicateJudgment(id) => write!(f, "duplicate judgment id {id}"),
            Self::DuplicateEvidenceUse { judgment, evidence } => {
                write!(
                    f,
                    "judgment {judgment} uses evidence {evidence} more than once"
                )
            }
            Self::NoJudgments(id) => write!(f, "inference {id} has no judgments"),
            Self::UnknownInferenceClass { inference, class } => {
                write!(f, "inference {inference} uses unknown class {class:?}")
            }
            Self::UnknownPurpose { judgment, purpose } => {
                write!(
                    f,
                    "judgment {judgment} uses unknown evidence purpose {purpose:?}"
                )
            }
            Self::Model(error) => write!(f, "invalid judgment script: {error}"),
        }
    }
}

impl std::error::Error for ScriptError {}

impl From<ModelError> for ScriptError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}
