use std::collections::BTreeSet;
use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(ModelError::EmptyIdentifier(stringify!($name)));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(EntityId);
id_type!(EvidenceId);
id_type!(EvidenceFamilyId);
id_type!(JudgmentId);
id_type!(ClaimId);
id_type!(BeliefId);
id_type!(InferenceRunId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceClass {
    UserAssertion,
    Metadata,
    Transcript,
    Ocr,
    ObjectDetection,
    SceneDetection,
    NamedEntity,
    FaceTrackReference,
    VoiceTrackReference,
}

impl EvidenceClass {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user_assertion" => Some(Self::UserAssertion),
            "metadata" => Some(Self::Metadata),
            "transcript" => Some(Self::Transcript),
            "ocr" => Some(Self::Ocr),
            "object_detection" => Some(Self::ObjectDetection),
            "scene_detection" => Some(Self::SceneDetection),
            "named_entity" => Some(Self::NamedEntity),
            "face_track_reference" => Some(Self::FaceTrackReference),
            "voice_track_reference" => Some(Self::VoiceTrackReference),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserAssertion => "user_assertion",
            Self::Metadata => "metadata",
            Self::Transcript => "transcript",
            Self::Ocr => "ocr",
            Self::ObjectDetection => "object_detection",
            Self::SceneDetection => "scene_detection",
            Self::NamedEntity => "named_entity",
            Self::FaceTrackReference => "face_track_reference",
            Self::VoiceTrackReference => "voice_track_reference",
        }
    }

    pub fn is_biometric_reference(self) -> bool {
        matches!(self, Self::FaceTrackReference | Self::VoiceTrackReference)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidencePurpose {
    DirectSupport,
    Corroboration,
    EntityLinking,
}

impl EvidencePurpose {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "direct_support" => Some(Self::DirectSupport),
            "corroboration" => Some(Self::Corroboration),
            "entity_linking" => Some(Self::EntityLinking),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DirectSupport => "direct_support",
            Self::Corroboration => "corroboration",
            Self::EntityLinking => "entity_linking",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InferenceClass {
    Descriptive,
    Preference,
    LocalEntityLink,
    CrossSourceAssociation,
    SensitiveTrait,
    RealWorldIdentity,
}

impl InferenceClass {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "descriptive" => Some(Self::Descriptive),
            "preference" => Some(Self::Preference),
            "local_entity_link" => Some(Self::LocalEntityLink),
            "cross_source_association" => Some(Self::CrossSourceAssociation),
            "sensitive_trait" => Some(Self::SensitiveTrait),
            "real_world_identity" => Some(Self::RealWorldIdentity),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Descriptive => "descriptive",
            Self::Preference => "preference",
            Self::LocalEntityLink => "local_entity_link",
            Self::CrossSourceAssociation => "cross_source_association",
            Self::SensitiveTrait => "sensitive_trait",
            Self::RealWorldIdentity => "real_world_identity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreSemantics {
    DetectorConfidence,
    ModelConfidence,
    ConditionalOptionProbability,
    Similarity,
    SoftTruth,
    PosteriorProbability,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    value: f64,
    semantics: ScoreSemantics,
}

impl Score {
    pub fn new(value: f64, semantics: ScoreSemantics) -> Result<Self, ModelError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ModelError::InvalidScore(value));
        }

        Ok(Self { value, semantics })
    }

    pub fn value(self) -> f64 {
        self.value
    }

    pub fn semantics(self) -> ScoreSemantics {
        self.semantics
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub repository: String,
    pub scope_id: String,
    pub record_id: String,
    pub revision: String,
}

impl SourceRef {
    pub fn new(
        repository: impl Into<String>,
        scope_id: impl Into<String>,
        record_id: impl Into<String>,
        revision: impl Into<String>,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            repository: non_empty("source.repository", repository.into())?,
            scope_id: non_empty("source.scope_id", scope_id.into())?,
            record_id: non_empty("source.record_id", record_id.into())?,
            revision: immutable_revision("source.revision", revision.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerRef {
    pub name: String,
    pub revision: String,
    pub model: Option<String>,
    pub config_hash: Option<String>,
}

impl ProducerRef {
    pub fn new(
        name: impl Into<String>,
        revision: impl Into<String>,
        model: Option<String>,
        config_hash: Option<String>,
    ) -> Result<Self, ModelError> {
        let model = optional_non_empty("producer.model", model)?;
        let config_hash = optional_non_empty("producer.config_hash", config_hash)?;

        Ok(Self {
            name: non_empty("producer.name", name.into())?,
            revision: immutable_revision("producer.revision", revision.into())?,
            model,
            config_hash,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub source: SourceRef,
    pub producer: ProducerRef,
    pub parent_evidence: BTreeSet<EvidenceId>,
}

impl Provenance {
    pub fn new(
        source: SourceRef,
        producer: ProducerRef,
        parent_evidence: impl IntoIterator<Item = EvidenceId>,
    ) -> Self {
        Self {
            source,
            producer,
            parent_evidence: parent_evidence.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceRef {
    pub id: EvidenceId,
    pub subject: Option<EntityId>,
    pub class: EvidenceClass,
    pub family: EvidenceFamilyId,
    pub score: Option<Score>,
    pub provenance: Provenance,
}

impl EvidenceRef {
    pub fn new(
        id: EvidenceId,
        subject: Option<EntityId>,
        class: EvidenceClass,
        family: EvidenceFamilyId,
        score: Option<Score>,
        provenance: Provenance,
    ) -> Self {
        Self {
            id,
            subject,
            class,
            family,
            score,
            provenance,
        }
    }

    pub fn has_complete_provenance(&self) -> bool {
        !self.provenance.source.repository.trim().is_empty()
            && !self.provenance.source.scope_id.trim().is_empty()
            && !self.provenance.source.record_id.trim().is_empty()
            && is_immutable_revision(&self.provenance.source.revision)
            && !self.provenance.producer.name.trim().is_empty()
            && is_immutable_revision(&self.provenance.producer.revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Predicate(String);

impl Predicate {
    pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
        Ok(Self(non_empty("predicate", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectValue {
    Entity(EntityId),
    Text(String),
    Boolean(bool),
}

impl ObjectValue {
    pub fn text(value: impl Into<String>) -> Result<Self, ModelError> {
        Ok(Self::Text(non_empty("object.text", value.into())?))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Proposition {
    pub subject: EntityId,
    pub predicate: Predicate,
    pub object: ObjectValue,
}

impl Proposition {
    pub fn new(subject: EntityId, predicate: Predicate, object: ObjectValue) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgmentSpecRef {
    pub name: String,
    pub revision: String,
}

impl JudgmentSpecRef {
    pub fn new(name: impl Into<String>, revision: impl Into<String>) -> Result<Self, ModelError> {
        Ok(Self {
            name: non_empty("judgment_spec.name", name.into())?,
            revision: non_empty("judgment_spec.revision", revision.into())?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgmentOutcome {
    Supports,
    Contradicts,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Judgment {
    pub id: JudgmentId,
    pub proposition: Proposition,
    pub outcome: JudgmentOutcome,
    pub confidence: Score,
    pub evidence: BTreeSet<EvidenceId>,
    pub spec: JudgmentSpecRef,
    pub model_revision: String,
}

impl Judgment {
    pub fn new(
        id: JudgmentId,
        proposition: Proposition,
        outcome: JudgmentOutcome,
        confidence: Score,
        evidence: impl IntoIterator<Item = EvidenceId>,
        spec: JudgmentSpecRef,
        model_revision: impl Into<String>,
    ) -> Result<Self, ModelError> {
        if !matches!(
            confidence.semantics(),
            ScoreSemantics::ModelConfidence | ScoreSemantics::ConditionalOptionProbability
        ) {
            return Err(ModelError::WrongScoreSemantics {
                context: "judgment confidence",
                expected: "model_confidence or conditional_option_probability",
                actual: confidence.semantics(),
            });
        }

        let evidence = evidence.into_iter().collect::<BTreeSet<_>>();
        if evidence.is_empty() {
            return Err(ModelError::MissingEvidence("judgment"));
        }

        Ok(Self {
            id,
            proposition,
            outcome,
            confidence,
            evidence,
            spec,
            model_revision: non_empty("judgment.model_revision", model_revision.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClaimOrigin {
    UserAssertion(EvidenceId),
    Judgment(JudgmentId),
    Rule {
        rule_id: String,
        input_claims: BTreeSet<ClaimId>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub id: ClaimId,
    pub proposition: Proposition,
    pub origin: ClaimOrigin,
}

impl Claim {
    pub fn from_user_assertion(
        id: ClaimId,
        proposition: Proposition,
        evidence: EvidenceId,
    ) -> Self {
        Self {
            id,
            proposition,
            origin: ClaimOrigin::UserAssertion(evidence),
        }
    }

    pub fn from_judgment(id: ClaimId, proposition: Proposition, judgment: JudgmentId) -> Self {
        Self {
            id,
            proposition,
            origin: ClaimOrigin::Judgment(judgment),
        }
    }

    pub fn from_rule(
        id: ClaimId,
        proposition: Proposition,
        rule_id: impl Into<String>,
        input_claims: impl IntoIterator<Item = ClaimId>,
    ) -> Result<Self, ModelError> {
        let input_claims = input_claims.into_iter().collect::<BTreeSet<_>>();
        if input_claims.is_empty() {
            return Err(ModelError::MissingEvidence("rule claim"));
        }

        Ok(Self {
            id,
            proposition,
            origin: ClaimOrigin::Rule {
                rule_id: non_empty("claim.rule_id", rule_id.into())?,
                input_claims,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Belief {
    pub id: BeliefId,
    pub claim: ClaimId,
    pub value: Score,
    pub inference_run: InferenceRunId,
}

impl Belief {
    pub fn new(
        id: BeliefId,
        claim: ClaimId,
        value: Score,
        inference_run: InferenceRunId,
    ) -> Result<Self, ModelError> {
        if !matches!(
            value.semantics(),
            ScoreSemantics::SoftTruth | ScoreSemantics::PosteriorProbability
        ) {
            return Err(ModelError::WrongScoreSemantics {
                context: "belief value",
                expected: "soft_truth or posterior_probability",
                actual: value.semantics(),
            });
        }

        Ok(Self {
            id,
            claim,
            value,
            inference_run,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    pub belief: BeliefId,
    pub rule_id: String,
    pub evidence: BTreeSet<EvidenceId>,
    pub judgments: BTreeSet<JudgmentId>,
    pub claims: BTreeSet<ClaimId>,
    pub inference_run: InferenceRunId,
}

impl Derivation {
    pub fn new(
        belief: BeliefId,
        rule_id: impl Into<String>,
        evidence: impl IntoIterator<Item = EvidenceId>,
        judgments: impl IntoIterator<Item = JudgmentId>,
        claims: impl IntoIterator<Item = ClaimId>,
        inference_run: InferenceRunId,
    ) -> Result<Self, ModelError> {
        let evidence = evidence.into_iter().collect::<BTreeSet<_>>();
        let judgments = judgments.into_iter().collect::<BTreeSet<_>>();
        let claims = claims.into_iter().collect::<BTreeSet<_>>();

        if evidence.is_empty() && judgments.is_empty() && claims.is_empty() {
            return Err(ModelError::MissingEvidence("belief derivation"));
        }

        Ok(Self {
            belief,
            rule_id: non_empty("derivation.rule_id", rule_id.into())?,
            evidence,
            judgments,
            claims,
            inference_run,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelError {
    EmptyIdentifier(&'static str),
    EmptyField(&'static str),
    InvalidScore(f64),
    InvalidRevision {
        field: &'static str,
        value: String,
    },
    MissingEvidence(&'static str),
    WrongScoreSemantics {
        context: &'static str,
        expected: &'static str,
        actual: ScoreSemantics,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier(name) => write!(f, "{name} cannot be empty"),
            Self::EmptyField(name) => write!(f, "{name} cannot be empty"),
            Self::InvalidScore(value) => {
                write!(f, "score must be finite and in [0, 1], got {value}")
            }
            Self::InvalidRevision { field, value } => write!(
                f,
                "{field} must be an immutable revision (sha256:, git:, commit:, or a full hex digest), got {value:?}"
            ),
            Self::MissingEvidence(context) => write!(f, "{context} requires at least one input"),
            Self::WrongScoreSemantics {
                context,
                expected,
                actual,
            } => write!(
                f,
                "{context} requires {expected} score semantics, got {actual:?}"
            ),
        }
    }
}

impl std::error::Error for ModelError {}

fn non_empty(field: &'static str, value: String) -> Result<String, ModelError> {
    if value.trim().is_empty() {
        Err(ModelError::EmptyField(field))
    } else {
        Ok(value)
    }
}

fn optional_non_empty(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<String>, ModelError> {
    value.map(|value| non_empty(field, value)).transpose()
}

fn immutable_revision(field: &'static str, value: String) -> Result<String, ModelError> {
    let value = non_empty(field, value)?;
    if is_immutable_revision(&value) {
        Ok(value)
    } else {
        Err(ModelError::InvalidRevision { field, value })
    }
}

fn is_immutable_revision(value: &str) -> bool {
    let prefixed = ["sha256:", "git:", "commit:"]
        .into_iter()
        .any(|prefix| value.starts_with(prefix) && value.len() > prefix.len());
    let hex = matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    prefixed || hex
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(value: &str) -> EntityId {
        EntityId::new(value).unwrap()
    }

    fn evidence_id(value: &str) -> EvidenceId {
        EvidenceId::new(value).unwrap()
    }

    fn proposition() -> Proposition {
        Proposition::new(
            entity("person:alice"),
            Predicate::new("uses").unwrap(),
            ObjectValue::text("Linux").unwrap(),
        )
    }

    #[test]
    fn scores_preserve_their_semantics() {
        let detector = Score::new(0.91, ScoreSemantics::DetectorConfidence).unwrap();
        let posterior = Score::new(0.91, ScoreSemantics::PosteriorProbability).unwrap();

        assert_eq!(detector.value(), posterior.value());
        assert_ne!(detector.semantics(), posterior.semantics());
    }

    #[test]
    fn scores_reject_invalid_probabilities() {
        assert!(matches!(
            Score::new(1.1, ScoreSemantics::ModelConfidence),
            Err(ModelError::InvalidScore(_))
        ));
        assert!(matches!(
            Score::new(f64::NAN, ScoreSemantics::ModelConfidence),
            Err(ModelError::InvalidScore(_))
        ));
    }

    #[test]
    fn movable_source_and_producer_revisions_are_rejected() {
        assert!(matches!(
            SourceRef::new("youtube-corpus", "video:1", "frame:42", "main"),
            Err(ModelError::InvalidRevision { .. })
        ));
        assert!(matches!(
            ProducerRef::new("visual-analysis", "latest", None, None),
            Err(ModelError::InvalidRevision { .. })
        ));
    }

    #[test]
    fn evidence_requires_pinned_source_and_producer_revisions() {
        assert!(SourceRef::new("youtube-corpus", "video:1", "frame:42", "").is_err());
        assert!(ProducerRef::new("visual-analysis", "", None, None).is_err());

        let provenance = Provenance::new(
            SourceRef::new(
                "youtube-corpus",
                "video:1",
                "video:1#frame:42",
                "sha256:abc",
            )
            .unwrap(),
            ProducerRef::new(
                "visual-analysis",
                "commit:123",
                Some("detr".into()),
                Some("config:456".into()),
            )
            .unwrap(),
            [],
        );
        let evidence = EvidenceRef::new(
            evidence_id("evidence:1"),
            Some(entity("person:alice")),
            EvidenceClass::ObjectDetection,
            EvidenceFamilyId::new("family:frame:42").unwrap(),
            Some(Score::new(0.93, ScoreSemantics::DetectorConfidence).unwrap()),
            provenance,
        );

        assert_eq!(evidence.provenance.producer.name, "visual-analysis");
        assert_eq!(evidence.family.as_str(), "family:frame:42");
    }

    #[test]
    fn judgment_confidence_cannot_be_a_posterior() {
        let result = Judgment::new(
            JudgmentId::new("judgment:1").unwrap(),
            proposition(),
            JudgmentOutcome::Supports,
            Score::new(0.8, ScoreSemantics::PosteriorProbability).unwrap(),
            [evidence_id("evidence:1")],
            JudgmentSpecRef::new("uses-linux", "v1").unwrap(),
            "jev:1",
        );

        assert!(matches!(
            result,
            Err(ModelError::WrongScoreSemantics {
                context: "judgment confidence",
                ..
            })
        ));
    }

    #[test]
    fn conditional_option_probabilities_are_judgment_semantics_not_belief_truth() {
        let confidence = Score::new(0.8, ScoreSemantics::ConditionalOptionProbability).unwrap();
        let judgment = Judgment::new(
            JudgmentId::new("judgment:conditional").unwrap(),
            proposition(),
            JudgmentOutcome::Supports,
            confidence,
            [evidence_id("evidence:1")],
            JudgmentSpecRef::new("support-check", "v1").unwrap(),
            "semif:model@revision",
        )
        .unwrap();

        assert_eq!(
            judgment.confidence.semantics(),
            ScoreSemantics::ConditionalOptionProbability
        );

        let belief = Belief::new(
            BeliefId::new("belief:conditional").unwrap(),
            ClaimId::new("claim:1").unwrap(),
            confidence,
            InferenceRunId::new("run:1").unwrap(),
        );
        assert!(matches!(
            belief,
            Err(ModelError::WrongScoreSemantics {
                context: "belief value",
                ..
            })
        ));
    }

    #[test]
    fn beliefs_accept_only_inference_semantics() {
        let result = Belief::new(
            BeliefId::new("belief:1").unwrap(),
            ClaimId::new("claim:1").unwrap(),
            Score::new(0.8, ScoreSemantics::ModelConfidence).unwrap(),
            InferenceRunId::new("run:1").unwrap(),
        );

        assert!(matches!(
            result,
            Err(ModelError::WrongScoreSemantics {
                context: "belief value",
                ..
            })
        ));
    }

    #[test]
    fn derivations_require_inputs() {
        let result = Derivation::new(
            BeliefId::new("belief:1").unwrap(),
            "rule:1",
            [],
            [],
            [],
            InferenceRunId::new("run:1").unwrap(),
        );

        assert!(matches!(
            result,
            Err(ModelError::MissingEvidence("belief derivation"))
        ));
    }
}
