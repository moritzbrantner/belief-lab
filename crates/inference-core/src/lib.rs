use std::collections::BTreeSet;
use std::fmt;

use belief_core::{
    Belief, BeliefId, Claim, Derivation, EvidenceFamilyId, EvidenceId, EvidencePurpose,
    EvidenceRef, InferenceClass, InferenceRunId, Judgment, JudgmentId, ModelError,
};
use belief_policy::{AuthorizationProfile, PolicyConfig};

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

    pub fn authorize(
        self,
        policy: &PolicyConfig,
    ) -> Result<AuthorizedInferenceRequest, AuthorizationError> {
        if !policy.allows_inference(self.class) {
            return Err(AuthorizationError::InferenceDenied(self.class));
        }

        let mut source_scopes = BTreeSet::new();
        let mut evidence_uses = BTreeSet::new();
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
                evidence_uses.insert((evidence.id.clone(), evidence_use.purpose));
            }
        }

        if source_scopes.len() > 1 && !policy.allow_cross_source_join {
            return Err(AuthorizationError::CrossSourceJoinDenied { source_scopes });
        }

        let authorization = AuthorizationReceipt {
            profile: policy.profile,
            inference_class: self.class,
            source_scopes: source_scopes.clone(),
            evidence_uses,
            cross_source_join: source_scopes.len() > 1,
        };

        Ok(AuthorizedInferenceRequest {
            inner: self,
            authorization,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationReceipt {
    profile: AuthorizationProfile,
    inference_class: InferenceClass,
    source_scopes: BTreeSet<(String, String)>,
    evidence_uses: BTreeSet<(EvidenceId, EvidencePurpose)>,
    cross_source_join: bool,
}

impl AuthorizationReceipt {
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
pub struct AuthorizedInferenceRequest {
    inner: InferenceRequest,
    authorization: AuthorizationReceipt,
}

impl AuthorizedInferenceRequest {
    pub fn request(&self) -> &InferenceRequest {
        &self.inner
    }

    pub fn authorization(&self) -> &AuthorizationReceipt {
        &self.authorization
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferenceResult {
    belief: Belief,
    derivation: Derivation,
    selected_judgments: BTreeSet<JudgmentId>,
    ignored_correlated_judgments: BTreeSet<JudgmentId>,
    authorization: AuthorizationReceipt,
}

impl InferenceResult {
    pub fn new(
        request: &AuthorizedInferenceRequest,
        belief: Belief,
        derivation: Derivation,
        selected_judgments: BTreeSet<JudgmentId>,
        ignored_correlated_judgments: BTreeSet<JudgmentId>,
    ) -> Result<Self, InferenceError> {
        let expected = request.request();

        if belief.id != expected.belief_id || belief.claim != expected.claim.id {
            return Err(InferenceError::InconsistentResult(
                "belief does not match authorized request".into(),
            ));
        }
        if belief.inference_run != expected.run_id {
            return Err(InferenceError::InconsistentResult(
                "belief inference run does not match authorized request".into(),
            ));
        }
        if derivation.belief != belief.id
            || derivation.inference_run != expected.run_id
            || derivation.rule_id != expected.rule_id
        {
            return Err(InferenceError::InconsistentResult(
                "derivation does not match authorized request".into(),
            ));
        }
        if derivation.judgments != selected_judgments {
            return Err(InferenceError::InconsistentResult(
                "derivation judgments do not match selected judgments".into(),
            ));
        }

        let available_judgments = expected
            .bases
            .iter()
            .map(|basis| basis.judgment.id.clone())
            .collect::<BTreeSet<_>>();
        if !selected_judgments.is_subset(&available_judgments)
            || !ignored_correlated_judgments.is_subset(&available_judgments)
            || !selected_judgments.is_disjoint(&ignored_correlated_judgments)
        {
            return Err(InferenceError::InconsistentResult(
                "result references judgments outside the authorized request".into(),
            ));
        }

        let expected_evidence = expected
            .bases
            .iter()
            .filter(|basis| selected_judgments.contains(&basis.judgment.id))
            .flat_map(|basis| basis.evidence.iter())
            .map(|evidence_use| evidence_use.evidence.id.clone())
            .collect::<BTreeSet<_>>();
        if derivation.evidence != expected_evidence {
            return Err(InferenceError::InconsistentResult(
                "derivation evidence does not match selected authorized judgments".into(),
            ));
        }

        Ok(Self {
            belief,
            derivation,
            selected_judgments,
            ignored_correlated_judgments,
            authorization: request.authorization().clone(),
        })
    }

    pub fn belief(&self) -> &Belief {
        &self.belief
    }

    pub fn derivation(&self) -> &Derivation {
        &self.derivation
    }

    pub fn selected_judgments(&self) -> &BTreeSet<JudgmentId> {
        &self.selected_judgments
    }

    pub fn ignored_correlated_judgments(&self) -> &BTreeSet<JudgmentId> {
        &self.ignored_correlated_judgments
    }

    pub fn authorization(&self) -> &AuthorizationReceipt {
        &self.authorization
    }

    pub fn into_parts(
        self,
    ) -> (
        Belief,
        Derivation,
        BTreeSet<JudgmentId>,
        BTreeSet<JudgmentId>,
        AuthorizationReceipt,
    ) {
        (
            self.belief,
            self.derivation,
            self.selected_judgments,
            self.ignored_correlated_judgments,
            self.authorization,
        )
    }
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
    InconsistentResult(String),
    Model(ModelError),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoUsableSignals => f.write_str(
                "inference request contains no usable supporting or contradicting signal",
            ),
            Self::InconsistentResult(reason) => {
                write!(f, "inconsistent inference result: {reason}")
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
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        let request = request(vec![basis(
            "judgment:1",
            evidence("transcript:1", EvidenceClass::Transcript, "video:1"),
        )]);

        assert!(request.authorize(&policy).is_ok());
    }

    #[test]
    fn policy_denies_unlisted_evidence_before_engine_execution() {
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        let request = request(vec![basis(
            "judgment:1",
            evidence("face:1", EvidenceClass::FaceTrackReference, "video:1"),
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
