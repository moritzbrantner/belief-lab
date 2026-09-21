use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const BELIEF_EVIDENCE_SCHEMA: &str = "belief_evidence_bundle";
pub const BELIEF_EVIDENCE_VERSION_V1: u32 = 1;
pub const SOURCE_SPAN_INTERCHANGE_SCHEMA: &str = "source_span_interchange";
pub const SOURCE_SPAN_INTERCHANGE_VERSION_V1: u32 = 1;
pub const MEDIA_EVIDENCE_SCHEMA: &str = "media_evidence";
pub const MEDIA_EVIDENCE_VERSION_V1: u32 = 1;

#[derive(Debug, Error, PartialEq)]
pub enum BeliefError {
    #[error("unsupported belief evidence schema `{schema}` version {version}")]
    UnsupportedBundle { schema: String, version: u32 },
    #[error("unsupported source span schema `{schema}` version {version}")]
    UnsupportedSourceSpans { schema: String, version: u32 },
    #[error("unsupported media evidence schema `{schema}` version {version}")]
    UnsupportedMediaEvidence { schema: String, version: u32 },
    #[error("identifier `{0}` must not be blank")]
    BlankIdentifier(String),
    #[error("duplicate evidence identifier `{0}`")]
    DuplicateEvidence(String),
    #[error("observation `{observation}` references unknown evidence `{reference}`")]
    UnknownEvidenceReference {
        observation: String,
        reference: String,
    },
    #[error("source span `{span}` references unknown source `{source_id}`")]
    UnknownSource { span: String, source_id: String },
    #[error("source span `{span}` has an invalid time range")]
    InvalidTimedSpan { span: String },
    #[error("media evidence `{id}` has an invalid time range")]
    InvalidMediaRange { id: String },
    #[error("score on `{id}` must be finite and between 0 and 1")]
    InvalidScore { id: String },
    #[error("claim `{0}` was not found")]
    ClaimNotFound(String),
    #[error("corpus identifier error: {0}")]
    CorpusIdentifier(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BeliefEvidenceBundleV1 {
    pub schema: String,
    pub schema_version: u32,
    pub source_spans: SourceSpanBatchV1,
    #[serde(default)]
    pub media_evidence: Vec<MediaEvidenceBatchV1>,
    #[serde(default)]
    pub observations: Vec<ObservationV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpanBatchV1 {
    pub schema: String,
    pub schema_version: u32,
    pub producer: ProducerV1,
    pub sources: Vec<SourceRecordV1>,
    pub spans: Vec<SourceSpanRecordV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProducerV1 {
    pub name: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRecordV1 {
    pub id: String,
    pub kind: String,
    pub revision: String,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub creators: Vec<String>,
    #[serde(default)]
    pub language: Option<String>,
    pub content_hash: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpanRecordV1 {
    pub id: String,
    pub source_id: String,
    pub sequence: u64,
    pub text: String,
    pub content_hash: String,
    #[serde(default)]
    pub language: Option<String>,
    pub locator: SourceLocatorV1,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SourceLocatorV1 {
    Text {
        byte_start: usize,
        byte_end: usize,
        #[serde(default)]
        paragraph_ordinal: Option<usize>,
        #[serde(default)]
        page: Option<u32>,
        #[serde(default)]
        section: Option<String>,
        #[serde(default)]
        source_selector: Option<String>,
        #[serde(default)]
        heading_path: Vec<String>,
    },
    Timed {
        segment_index: u64,
        #[serde(default)]
        start_seconds: Option<f64>,
        #[serde(default)]
        end_seconds: Option<f64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaEvidenceBatchV1 {
    pub schema: String,
    pub schema_version: u32,
    pub producer: ProducerV1,
    pub video: MediaEvidenceVideoV1,
    pub revision: String,
    #[serde(default)]
    pub scenes: Vec<SceneEvidenceV1>,
    #[serde(default)]
    pub ocr_observations: Vec<OcrObservationEvidenceV1>,
    #[serde(default)]
    pub ocr_tracks: Vec<OcrTrackEvidenceV1>,
    #[serde(default)]
    pub sponsorblock: Option<SponsorBlockEvidenceV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaEvidenceVideoV1 {
    pub id: String,
    #[serde(default)]
    pub youtube_id: Option<String>,
    pub source_url: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingEvidenceV1 {
    pub run_id: String,
    pub processor: String,
    pub processor_version: String,
    pub model: String,
    pub model_version: String,
    pub input_hash: String,
    pub config_hash: String,
    pub processing_config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneEvidenceV1 {
    pub id: String,
    pub scene_index: u64,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub metadata: Value,
    pub provenance: ProcessingEvidenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaBoundingBoxV1 {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrObservationEvidenceV1 {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub frame_index: Option<u64>,
    #[serde(default)]
    pub timestamp_seconds: Option<f64>,
    #[serde(default)]
    pub scene_index: Option<u64>,
    #[serde(default)]
    pub region: Option<MediaBoundingBoxV1>,
    #[serde(default)]
    pub confidence: Option<f64>,
    pub attributes: Value,
    pub provenance: ProcessingEvidenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrTrackEvidenceV1 {
    pub id: String,
    pub text: String,
    pub role: String,
    #[serde(default)]
    pub language: Option<String>,
    pub sample_count: u32,
    #[serde(default)]
    pub start_frame: Option<u64>,
    #[serde(default)]
    pub end_frame: Option<u64>,
    #[serde(default)]
    pub start_seconds: Option<f64>,
    #[serde(default)]
    pub end_seconds: Option<f64>,
    #[serde(default)]
    pub region: Option<MediaBoundingBoxV1>,
    pub metadata: Value,
    #[serde(default)]
    pub observation_ids: Vec<String>,
    #[serde(default)]
    pub scene_ids: Vec<String>,
    pub provenance: ProcessingEvidenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SponsorBlockEvidenceV1 {
    pub snapshot_id: String,
    pub response_hash: String,
    pub data_license: String,
    pub attribution: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub segments: Vec<SponsorBlockSegmentEvidenceV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SponsorBlockSegmentEvidenceV1 {
    pub uuid: String,
    pub category: String,
    #[serde(default)]
    pub action_type: Option<String>,
    pub start_seconds: f64,
    pub end_seconds: f64,
    #[serde(default)]
    pub video_duration: Option<f64>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKindV1 {
    ObjectDetection,
    FaceTrack,
    VoiceTrack,
    NamedEntity,
    Relation,
    EntityLink,
    ModelJudgment,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoreSemanticsV1 {
    DetectionConfidence,
    TranscriptionConfidence,
    NerConfidence,
    EntityLinkProbability,
    JevConfidence,
    HeuristicStrength,
    SoftTruth,
    PosteriorProbability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceScoreV1 {
    pub value: f64,
    pub semantics: ScoreSemanticsV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObservationV1 {
    pub id: String,
    pub kind: ObservationKindV1,
    pub subject: String,
    pub predicate: String,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub score: Option<EvidenceScoreV1>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub correlation_group: Option<String>,
    pub provenance: ProducerV1,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub support: EvidenceScoreV1,
    pub inference_rule: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub score: Option<EvidenceScoreV1>,
    #[serde(default)]
    pub producer: Option<ProducerV1>,
    #[serde(default)]
    pub correlation_group: Option<String>,
    #[serde(default)]
    pub children: Vec<ExplanationNode>,
}
