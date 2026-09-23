use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use belief_core::{
    EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId, EvidencePurpose, EvidenceRef,
    ProducerRef, Provenance, Score, ScoreSemantics, SourceRef,
};
use belief_policy::{AuthorizationProfile, PolicyConfig};
use serde::Deserialize;

pub const EVIDENCE_INTERCHANGE_SCHEMA: &str = "belief_evidence_interchange";
pub const EVIDENCE_INTERCHANGE_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceBatchV1 {
    schema: String,
    schema_version: u32,
    exporter: ExporterV1,
    revision: String,
    #[serde(default)]
    evidence: Vec<EvidenceRecordV1>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExporterV1 {
    name: String,
    revision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceRecordV1 {
    id: String,
    #[serde(default)]
    subject: Option<String>,
    class: String,
    family_id: String,
    source: SourceV1,
    producer: ProducerV1,
    #[serde(default)]
    score: Option<ScoreV1>,
    #[serde(default)]
    parent_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceV1 {
    repository: String,
    scope_id: String,
    record_id: String,
    revision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProducerV1 {
    name: String,
    revision: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    config_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScoreV1 {
    value: f64,
    semantics: ScoreSemanticsV1,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScoreSemanticsV1 {
    DetectorConfidence,
    ModelConfidence,
    Similarity,
    SoftTruth,
    PosteriorProbability,
}

impl ScoreSemanticsV1 {
    fn to_core(self) -> ScoreSemantics {
        match self {
            Self::DetectorConfidence => ScoreSemantics::DetectorConfidence,
            Self::ModelConfidence => ScoreSemantics::ModelConfidence,
            Self::Similarity => ScoreSemantics::Similarity,
            Self::SoftTruth => ScoreSemantics::SoftTruth,
            Self::PosteriorProbability => ScoreSemantics::PosteriorProbability,
        }
    }

    fn is_inference_result(self) -> bool {
        matches!(self, Self::SoftTruth | Self::PosteriorProbability)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedEvidenceBatch {
    exporter_name: String,
    exporter_revision: String,
    revision: String,
    evidence: Vec<EvidenceRef>,
}

impl ValidatedEvidenceBatch {
    pub fn parse_json(json: &str) -> Result<Self, InterchangeError> {
        let batch = serde_json::from_str::<EvidenceBatchV1>(json)
            .map_err(|error| InterchangeError::InvalidJson(error.to_string()))?;
        validate_batch(batch)
    }

    pub fn exporter_name(&self) -> &str {
        &self.exporter_name
    }

    pub fn exporter_revision(&self) -> &str {
        &self.exporter_revision
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn evidence(&self) -> &[EvidenceRef] {
        &self.evidence
    }

    pub fn authorize(
        self,
        policy: &PolicyConfig,
    ) -> Result<AuthorizedEvidenceBatch, ImportAuthorizationError> {
        let mut classes = BTreeSet::new();
        let mut evidence_ids = BTreeSet::new();

        for evidence in &self.evidence {
            if !evidence_is_admissible(policy, evidence) {
                return Err(ImportAuthorizationError::EvidenceClassDenied {
                    evidence: evidence.id.clone(),
                    class: evidence.class,
                });
            }
            classes.insert(evidence.class);
            evidence_ids.insert(evidence.id.clone());
        }

        let receipt = EvidenceImportReceipt {
            profile: policy.profile(),
            exporter_name: self.exporter_name.clone(),
            exporter_revision: self.exporter_revision.clone(),
            batch_revision: self.revision.clone(),
            evidence_ids,
            classes,
        };

        Ok(AuthorizedEvidenceBatch {
            batch: self,
            receipt,
        })
    }
}

fn evidence_is_admissible(policy: &PolicyConfig, evidence: &EvidenceRef) -> bool {
    [
        EvidencePurpose::DirectSupport,
        EvidencePurpose::Corroboration,
        EvidencePurpose::EntityLinking,
    ]
    .into_iter()
    .any(|purpose| policy.allows_evidence_ref(evidence, purpose))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceImportReceipt {
    profile: AuthorizationProfile,
    exporter_name: String,
    exporter_revision: String,
    batch_revision: String,
    evidence_ids: BTreeSet<EvidenceId>,
    classes: BTreeSet<EvidenceClass>,
}

impl EvidenceImportReceipt {
    pub fn profile(&self) -> AuthorizationProfile {
        self.profile
    }

    pub fn exporter_name(&self) -> &str {
        &self.exporter_name
    }

    pub fn exporter_revision(&self) -> &str {
        &self.exporter_revision
    }

    pub fn batch_revision(&self) -> &str {
        &self.batch_revision
    }

    pub fn evidence_ids(&self) -> &BTreeSet<EvidenceId> {
        &self.evidence_ids
    }

    pub fn classes(&self) -> &BTreeSet<EvidenceClass> {
        &self.classes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorizedEvidenceBatch {
    batch: ValidatedEvidenceBatch,
    receipt: EvidenceImportReceipt,
}

impl AuthorizedEvidenceBatch {
    pub fn evidence(&self) -> &[EvidenceRef] {
        &self.batch.evidence
    }

    pub fn receipt(&self) -> &EvidenceImportReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> (Vec<EvidenceRef>, EvidenceImportReceipt) {
        (self.batch.evidence, self.receipt)
    }
}

fn validate_batch(batch: EvidenceBatchV1) -> Result<ValidatedEvidenceBatch, InterchangeError> {
    if batch.schema != EVIDENCE_INTERCHANGE_SCHEMA
        || batch.schema_version != EVIDENCE_INTERCHANGE_VERSION_V1
    {
        return Err(InterchangeError::UnsupportedSchema {
            schema: batch.schema,
            version: batch.schema_version,
        });
    }

    let exporter_name = required("exporter.name", batch.exporter.name)?;
    let exporter_revision = required("exporter.revision", batch.exporter.revision)?;
    let revision = required("revision", batch.revision)?;

    let mut records = BTreeMap::<EvidenceId, EvidenceRecordV1>::new();
    for record in batch.evidence {
        let id = EvidenceId::new(record.id.clone()).map_err(InterchangeError::Model)?;
        if records.insert(id.clone(), record).is_some() {
            return Err(InterchangeError::DuplicateEvidence(id));
        }
    }

    let known_ids = records.keys().cloned().collect::<BTreeSet<_>>();
    let mut parents = BTreeMap::<EvidenceId, BTreeSet<EvidenceId>>::new();
    for (id, record) in &records {
        let mut record_parents = BTreeSet::new();
        for parent in &record.parent_evidence_ids {
            let parent = EvidenceId::new(parent.clone()).map_err(InterchangeError::Model)?;
            if parent == *id {
                return Err(InterchangeError::EvidenceCycle(id.clone()));
            }
            if !known_ids.contains(&parent) {
                return Err(InterchangeError::MissingParent {
                    evidence: id.clone(),
                    parent,
                });
            }
            record_parents.insert(parent);
        }
        parents.insert(id.clone(), record_parents);
    }

    let ordered_ids = topological_order(&parents)?;
    let mut evidence = Vec::with_capacity(ordered_ids.len());

    for id in ordered_ids {
        let record = records
            .remove(&id)
            .expect("topological order must reference validated evidence");

        let class = EvidenceClass::parse(&record.class).ok_or_else(|| {
            InterchangeError::UnknownEvidenceClass {
                evidence: id.clone(),
                class: record.class.clone(),
            }
        })?;

        let subject = record
            .subject
            .map(EntityId::new)
            .transpose()
            .map_err(InterchangeError::Model)?;
        let family = EvidenceFamilyId::new(record.family_id).map_err(InterchangeError::Model)?;

        let source = SourceRef::new(
            record.source.repository,
            record.source.scope_id,
            record.source.record_id,
            record.source.revision,
        )
        .map_err(InterchangeError::Model)?;

        let producer = ProducerRef::new(
            record.producer.name,
            record.producer.revision,
            record.producer.model,
            record.producer.config_hash,
        )
        .map_err(InterchangeError::Model)?;

        let score = record
            .score
            .map(|score| {
                if score.semantics.is_inference_result() {
                    return Err(InterchangeError::InferenceScoreImported {
                        evidence: id.clone(),
                        semantics: score.semantics.to_core(),
                    });
                }
                Score::new(score.value, score.semantics.to_core()).map_err(InterchangeError::Model)
            })
            .transpose()?;

        let parent_evidence = parents
            .get(&id)
            .expect("parent map must contain every validated evidence record")
            .clone();

        evidence.push(EvidenceRef::new(
            id,
            subject,
            class,
            family,
            score,
            Provenance::new(source, producer, parent_evidence),
        ));
    }

    Ok(ValidatedEvidenceBatch {
        exporter_name,
        exporter_revision,
        revision,
        evidence,
    })
}

fn topological_order(
    parents: &BTreeMap<EvidenceId, BTreeSet<EvidenceId>>,
) -> Result<Vec<EvidenceId>, InterchangeError> {
    let mut remaining = parents.clone();
    let mut emitted = BTreeSet::<EvidenceId>::new();
    let mut order = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.is_subset(&emitted))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        if ready.is_empty() {
            let id = remaining
                .keys()
                .next()
                .expect("non-empty map must have a first key")
                .clone();
            return Err(InterchangeError::EvidenceCycle(id));
        }

        for id in ready {
            remaining.remove(&id);
            emitted.insert(id.clone());
            order.push(id);
        }
    }

    Ok(order)
}

fn required(field: &'static str, value: String) -> Result<String, InterchangeError> {
    if value.trim().is_empty() {
        Err(InterchangeError::EmptyField(field))
    } else {
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterchangeError {
    InvalidJson(String),
    UnsupportedSchema {
        schema: String,
        version: u32,
    },
    EmptyField(&'static str),
    DuplicateEvidence(EvidenceId),
    MissingParent {
        evidence: EvidenceId,
        parent: EvidenceId,
    },
    EvidenceCycle(EvidenceId),
    UnknownEvidenceClass {
        evidence: EvidenceId,
        class: String,
    },
    InferenceScoreImported {
        evidence: EvidenceId,
        semantics: ScoreSemantics,
    },
    Model(belief_core::ModelError),
}

impl fmt::Display for InterchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid evidence interchange JSON: {error}"),
            Self::UnsupportedSchema { schema, version } => {
                write!(f, "unsupported evidence interchange {schema}@{version}")
            }
            Self::EmptyField(field) => write!(f, "{field} cannot be empty"),
            Self::DuplicateEvidence(id) => write!(f, "duplicate evidence id {id}"),
            Self::MissingParent { evidence, parent } => {
                write!(f, "evidence {evidence} references missing parent {parent}")
            }
            Self::EvidenceCycle(id) => {
                write!(
                    f,
                    "evidence dependency graph contains a cycle involving {id}"
                )
            }
            Self::UnknownEvidenceClass { evidence, class } => {
                write!(f, "evidence {evidence} uses unknown class {class:?}")
            }
            Self::InferenceScoreImported {
                evidence,
                semantics,
            } => write!(
                f,
                "evidence {evidence} cannot import inference-result score semantics {semantics:?}"
            ),
            Self::Model(error) => write!(f, "invalid evidence model: {error}"),
        }
    }
}

impl std::error::Error for InterchangeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportAuthorizationError {
    EvidenceClassDenied {
        evidence: EvidenceId,
        class: EvidenceClass,
    },
}

impl fmt::Display for ImportAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EvidenceClassDenied { evidence, class } => write!(
                f,
                "evidence {evidence} ({}) is not authorized for import under the active policy",
                class.as_str()
            ),
        }
    }
}

impl std::error::Error for ImportAuthorizationError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_json(records: &str) -> String {
        format!(
            r#"{{
  "schema": "belief_evidence_interchange",
  "schemaVersion": 1,
  "exporter": {{
    "name": "youtube-corpus",
    "revision": "git:exporter-123"
  }},
  "revision": "batch:abc",
  "evidence": [{records}]
}}"#
        )
    }

    fn transcript_record() -> &'static str {
        r#"{
  "id": "evidence:transcript:1",
  "subject": "corpus-person:1",
  "class": "transcript",
  "familyId": "family:utterance:1",
  "source": {
    "repository": "youtube-corpus",
    "scopeId": "video:1",
    "recordId": "transcript-segment:1",
    "revision": "sha256:transcript-1"
  },
  "producer": {
    "name": "audio-analysis",
    "revision": "git:audio-123",
    "model": "whisper",
    "configHash": "sha256:config-1"
  },
  "score": {
    "value": 0.91,
    "semantics": "model_confidence"
  }
}"#
    }

    fn named_entity_record(parent: &str) -> String {
        format!(
            r#"{{
  "id": "evidence:entity:1",
  "subject": "corpus-person:1",
  "class": "named_entity",
  "familyId": "family:utterance:1",
  "source": {{
    "repository": "youtube-corpus",
    "scopeId": "video:1",
    "recordId": "named-entity:1",
    "revision": "sha256:entity-1"
  }},
  "producer": {{
    "name": "nlp-stack",
    "revision": "git:nlp-123",
    "model": "ner-v1"
  }},
  "score": {{
    "value": 0.84,
    "semantics": "model_confidence"
  }},
  "parentEvidenceIds": ["{parent}"]
}}"#
        )
    }

    #[test]
    fn multimodal_fixture_requires_the_biometric_import_gate() {
        let json =
            include_str!("../../../fixtures/evidence-interchange/youtube-multimodal-v1.json");
        let batch = ValidatedEvidenceBatch::parse_json(json).unwrap();

        assert_eq!(batch.evidence().len(), 6);
        assert_eq!(batch.exporter_name(), "youtube-corpus");

        let semantic =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        assert!(matches!(
            batch.clone().authorize(&semantic),
            Err(ImportAuthorizationError::EvidenceClassDenied { .. })
        ));

        let multimodal = PolicyConfig::from_pairs([
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
        ])
        .unwrap();
        let authorized = batch.authorize(&multimodal).unwrap();

        assert!(authorized
            .receipt()
            .classes()
            .contains(&EvidenceClass::Transcript));
        assert!(authorized
            .receipt()
            .classes()
            .contains(&EvidenceClass::NamedEntity));
        assert!(authorized
            .receipt()
            .classes()
            .contains(&EvidenceClass::FaceTrackReference));
        assert!(authorized
            .receipt()
            .classes()
            .contains(&EvidenceClass::VoiceTrackReference));
    }

    #[test]
    fn parses_and_topologically_orders_producer_owned_references() {
        let entity = named_entity_record("evidence:transcript:1");
        let json = base_json(&format!("{entity},{}", transcript_record()));

        let batch = ValidatedEvidenceBatch::parse_json(&json).unwrap();

        assert_eq!(batch.exporter_name(), "youtube-corpus");
        assert_eq!(batch.exporter_revision(), "git:exporter-123");
        assert_eq!(batch.evidence().len(), 2);
        assert_eq!(batch.evidence()[0].id.as_str(), "evidence:transcript:1");
        assert_eq!(batch.evidence()[1].id.as_str(), "evidence:entity:1");
        assert_eq!(batch.evidence()[1].provenance.producer.name(), "nlp-stack");
        assert!(batch.evidence()[1]
            .provenance
            .parent_evidence
            .contains(&EvidenceId::new("evidence:transcript:1").unwrap()));
    }

    #[test]
    fn rejects_unknown_schema_versions() {
        let json = base_json(transcript_record())
            .replace(r#""schemaVersion": 1"#, r#""schemaVersion": 2"#);

        assert!(matches!(
            ValidatedEvidenceBatch::parse_json(&json),
            Err(InterchangeError::UnsupportedSchema { version: 2, .. })
        ));
    }

    #[test]
    fn rejects_missing_parent_references() {
        let entity = named_entity_record("evidence:missing");
        let json = base_json(&entity);

        assert!(matches!(
            ValidatedEvidenceBatch::parse_json(&json),
            Err(InterchangeError::MissingParent { .. })
        ));
    }

    #[test]
    fn rejects_dependency_cycles() {
        let first = named_entity_record("evidence:entity:2");
        let second = named_entity_record("evidence:entity:1")
            .replace("evidence:entity:1", "evidence:entity:2");
        let json = base_json(&format!("{first},{second}"));

        assert!(matches!(
            ValidatedEvidenceBatch::parse_json(&json),
            Err(InterchangeError::EvidenceCycle(_))
        ));
    }

    #[test]
    fn imported_evidence_cannot_claim_posterior_semantics() {
        let json =
            base_json(&transcript_record().replace("model_confidence", "posterior_probability"));

        assert!(matches!(
            ValidatedEvidenceBatch::parse_json(&json),
            Err(InterchangeError::InferenceScoreImported { .. })
        ));
    }

    #[test]
    fn policy_controls_whether_biometric_references_are_admitted() {
        let face = transcript_record()
            .replace("evidence:transcript:1", "evidence:face:1")
            .replace(
                r#""class": "transcript""#,
                r#""class": "face_track_reference""#,
            )
            .replace("transcript-segment:1", "face-track:1");
        let batch = ValidatedEvidenceBatch::parse_json(&base_json(&face)).unwrap();

        let semantic =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        assert!(matches!(
            batch.clone().authorize(&semantic),
            Err(ImportAuthorizationError::EvidenceClassDenied { .. })
        ));

        let multimodal_without_opt_in =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "multimodal_research")]).unwrap();
        assert!(matches!(
            batch.clone().authorize(&multimodal_without_opt_in),
            Err(ImportAuthorizationError::EvidenceClassDenied { .. })
        ));

        let multimodal = PolicyConfig::from_pairs([
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
        ])
        .unwrap();
        let authorized = batch.authorize(&multimodal).unwrap();

        assert_eq!(
            authorized.receipt().profile(),
            AuthorizationProfile::MultimodalResearch
        );
        assert!(authorized
            .receipt()
            .classes()
            .contains(&EvidenceClass::FaceTrackReference));
    }

    #[test]
    fn authorization_receipt_pins_exporter_and_batch_revisions() {
        let batch = ValidatedEvidenceBatch::parse_json(&base_json(transcript_record())).unwrap();
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();

        let authorized = batch.authorize(&policy).unwrap();

        assert_eq!(authorized.receipt().exporter_name(), "youtube-corpus");
        assert_eq!(authorized.receipt().exporter_revision(), "git:exporter-123");
        assert_eq!(authorized.receipt().batch_revision(), "batch:abc");
        assert_eq!(authorized.receipt().evidence_ids().len(), 1);
    }
}
