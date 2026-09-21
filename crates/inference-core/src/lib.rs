use std::collections::BTreeSet;
use std::fmt;

use belief_core::{
    Belief, BeliefId, Claim, Derivation, EvidenceFamilyId, EvidenceId, EvidencePurpose,
    EvidenceRef, InferenceClass, InferenceRunId, Judgment, JudgmentId, ModelError,
};
use belief_policy::PolicyConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceUse {
    pub evidence: EvidenceRef,
    pub purpose: EvidencePurpose,
}

impl EvidenceUse {
    pub fn new(evidence: EvidenceRef, purpose: EvidencePurpose) -> Self {
        Self { evidence, purpose }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JudgmentBasis {
    pub judgment: Judgment,
    pub correlation_group: EvidenceFamilyId,
    pub evidence: Vec<EvidenceUse>,
}

impl JudgmentBasis {
    pub fn new(
        judgment: Judgment,
        correlation_group: EvidenceFamilyId,
        evidence: Vec<EvidenceUse>,
    ) -> Result<Self, RequestError> {
        let expected = judgment.evidence.clone();
        let actual = evidence
            .iter()
            .map(|item| item.evidence.id.clone())
            .collect::<BTreeSet<_>>();

        if expected != actual {
            return Err(RequestError::BasisEvidenceMismatch {
                judgment: judgment.id.clone(),
                expected,
                actual,
            });
        }

        Ok(Self {
            judgment,
            correlation_group,
            evidence,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferenceRequest {
    pub run_id: InferenceRunId,
    pub belief_id: BeliefId,
    pub class: InferenceClass,
    pub claim: Claim,
    pub rule_id: String,
    pub bases: Vec<JudgmentBasis>,
}

impl InferenceRequest {
    pub fn new(
        run_id: InferenceRunId,
        belief_id: BeliefId,
        class: InferenceClass,
        claim: Claim,
        rule_id: impl Into<String>,
        bases: Vec<JudgmentBasis>,
    ) -> Result<Self, RequestError> {
        let rule_id = rule_id.into();
        if rule_id.trim().is_empty() {
            return Err(RequestError::EmptyRuleId);
        }
        if bases.is_empty() {
            return Err(RequestError::NoJudgmentBases);
        }

        Ok(Self {
            run_id,
            belief_id,
            class,
            claim,
            rule_id,
            bases,
        })
    }

    pub fn authorize(self, policy: &PolicyConfig) -> Result<AuthorizedInferenceRequest, AuthorizationError> {
        if !policy.allows_inference(self.class) {
            return Err(AuthorizationError::InferenceDenied(self.class));
        }

        let mut source_scopes = BTreeSet::new();
        for basis in &self.bases {
            for evidence_use in &basis.evidence {
                let evidence = &evidence_use.evidence;
                if !policy.allows_evidence(evidence.class, evidence_use.purpose) {
                    return Err(AuthorizationError::EvidenceDenied {
                        evidence: evidence.id.clone(),
                        class: evidence.class,
                        purpose: evidence_use.purpose,
                    });
                }
                source_scopes.insert((
                    evidence.provenance.source.repository.clone(),
                    evidence.provenance.source.scope_id.clone(),
                ));
            }
        }

        if source_scopes.len() > 1 && !policy.allow_cross_source_join {
            return Err(AuthorizationError::CrossSourceJoinDenied { source_scopes });
        }

        Ok(AuthorizedInferenceRequest { inner: self })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorizedInferenceRequest {
    inner: InferenceRequest,
}

impl AuthorizedInferenceRequest {
    pub fn request(&self) -> &InferenceRequest {
        &self.inner
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferenceResult {
    pub belief: Belief,
    pub derivation: Derivation,
    pub selected_judgments: BTreeSet<JudgmentId>,
    pub ignored_correlated_judgments: BTreeSet<JudgmentId>,
}

pub trait InferenceEngine {
    fn name(&self) -> &'static str;

    fn infer(
        &self,
        request: &AuthorizedInferenceRequest,
    ) -> Result<InferenceResult, InferenceError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestError {
    EmptyRuleId,
    NoJudgmentBases,
    BasisEvidenceMismatch {
        judgment: JudgmentId,
        expected: BTreeSet<EvidenceId>,
        actual: BTreeSet<EvidenceId>,
    },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRuleId => f.write_str("inference rule id cannot be empty"),
            Self::NoJudgmentBases => {
                f.write_str("an inference request requires at least one judgment basis")
            }
            Self::BasisEvidenceMismatch {
                judgment,
                expected,
                actual,
            } => write!(
                f,
                "judgment {judgment} references evidence {expected:?}, but basis supplied {actual:?}"
            ),
        }
    }
}

impl std::error::Error for RequestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationError {
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

impl fmt::Display for AuthorizationError {
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

impl std::error::Error for AuthorizationError {}

#[derive(Debug, Clone, PartialEq)]
pub enum InferenceError {
    NoUsableSignals,
    Model(ModelError),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUsableSignals => {
                f.write_str("inference request contains no usable supporting or contradicting signal")
            }
            Self::Model(error) => write!(f, "could not construct inference result: {error}"),
        }
    }
}

impl std::error::Error for InferenceError {}

impl From<ModelError> for InferenceError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}

#[cfg(test)]
mod tests {
    use belief_core::{
        ClaimId, EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId, JudgmentId,
        JudgmentOutcome, JudgmentSpecRef, ObjectValue, Predicate, ProducerRef, Proposition,
        Provenance, Score, ScoreSemantics, SourceRef,
    };

    use super::*;

    fn evidence(id: &str, class: EvidenceClass, scope: &str) -> EvidenceRef {
        EvidenceRef::new(
            EvidenceId::new(id).unwrap(),
            Some(EntityId::new("person:alice").unwrap()),
            class,
            EvidenceFamilyId::new(format!("family:{id}")).unwrap(),
            None,
            Provenance::new(
                SourceRef::new(
                    "youtube-corpus",
                    scope,
                    format!("{scope}#{id}"),
                    "sha256:source",
                )
                .unwrap(),
                ProducerRef::new("fixture", "commit:1", None, Some("config:1".into())).unwrap(),
                [],
            ),
        )
    }

    fn claim() -> Claim {
        Claim::from_user_assertion(
            ClaimId::new("claim:1").unwrap(),
            Proposition::new(
                EntityId::new("person:alice").unwrap(),
                Predicate::new("prefers_customization").unwrap(),
                ObjectValue::Boolean(true),
            ),
            EvidenceId::new("assertion:1").unwrap(),
        )
    }

    fn basis(id: &str, evidence_ref: EvidenceRef) -> JudgmentBasis {
        let evidence_id = evidence_ref.id.clone();
        let judgment = Judgment::new(
            JudgmentId::new(id).unwrap(),
            claim().proposition,
            JudgmentOutcome::Supports,
            Score::new(0.8, ScoreSemantics::ModelConfidence).unwrap(),
            [evidence_id],
            JudgmentSpecRef::new("fixture", "v1").unwrap(),
            "fixture-model:1",
        )
        .unwrap();

        JudgmentBasis::new(
            judgment,
            EvidenceFamilyId::new(format!("correlation:{id}")).unwrap(),
            vec![EvidenceUse::new(
                evidence_ref,
                EvidencePurpose::Corroboration,
            )],
        )
        .unwrap()
    }

    fn request(bases: Vec<JudgmentBasis>) -> InferenceRequest {
        InferenceRequest::new(
            InferenceRunId::new("run:1").unwrap(),
            BeliefId::new("belief:1").unwrap(),
            InferenceClass::Descriptive,
            claim(),
            "baseline:v1",
            bases,
        )
        .unwrap()
    }

    #[test]
    fn semantic_policy_authorizes_semantic_evidence() {
        let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")])
            .unwrap();
        let request = request(vec![basis(
            "judgment:1",
            evidence("transcript:1", EvidenceClass::Transcript, "video:1"),
        )]);

        assert!(request.authorize(&policy).is_ok());
    }

    #[test]
    fn policy_denies_unlisted_evidence_before_engine_execution() {
        let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")])
            .unwrap();
        let request = request(vec![basis(
            "judgment:1",
            evidence(
                "face:1",
                EvidenceClass::FaceTrackReference,
                "video:1",
            ),
        )]);

        assert!(matches!(
            request.authorize(&policy),
            Err(AuthorizationError::EvidenceDenied { .. })
        ));
    }

    #[test]
    fn cross_source_join_requires_independent_opt_in() {
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "multimodal_research")]).unwrap();
        let request = request(vec![
            basis(
                "judgment:1",
                evidence("transcript:1", EvidenceClass::Transcript, "video:1"),
            ),
            basis(
                "judgment:2",
                evidence("transcript:2", EvidenceClass::Transcript, "video:2"),
            ),
        ]);

        assert!(matches!(
            request.authorize(&policy),
            Err(AuthorizationError::CrossSourceJoinDenied { .. })
        ));
    }

    #[test]
    fn judgment_basis_must_supply_exact_judgment_evidence() {
        let evidence_ref = evidence("transcript:1", EvidenceClass::Transcript, "video:1");
        let judgment = Judgment::new(
            JudgmentId::new("judgment:1").unwrap(),
            claim().proposition,
            JudgmentOutcome::Supports,
            Score::new(0.8, ScoreSemantics::ModelConfidence).unwrap(),
            [EvidenceId::new("different").unwrap()],
            JudgmentSpecRef::new("fixture", "v1").unwrap(),
            "fixture-model:1",
        )
        .unwrap();

        assert!(matches!(
            JudgmentBasis::new(
                judgment,
                EvidenceFamilyId::new("correlation:1").unwrap(),
                vec![EvidenceUse::new(
                    evidence_ref,
                    EvidencePurpose::Corroboration
                )],
            ),
            Err(RequestError::BasisEvidenceMismatch { .. })
        ));
    }
}
