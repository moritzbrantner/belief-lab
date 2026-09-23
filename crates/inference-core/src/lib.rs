use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

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
        let basis = Self {
            judgment,
            correlation_group,
            evidence,
        };
        basis.validate()?;
        Ok(basis)
    }

    fn validate(&self) -> Result<(), RequestError> {
        let expected = self.judgment.evidence();
        let actual = self
            .evidence
            .iter()
            .map(|item| item.evidence.id.clone())
            .collect::<BTreeSet<_>>();

        if expected != &actual {
            return Err(RequestError::BasisEvidenceMismatch {
                judgment: self.judgment.id().clone(),
                expected: expected.clone(),
                actual,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustedInferenceRule {
    BaselineDescriptiveV1,
    BaselinePreferenceV1,
}

impl TrustedInferenceRule {
    pub fn class(self) -> InferenceClass {
        match self {
            Self::BaselineDescriptiveV1 => InferenceClass::Descriptive,
            Self::BaselinePreferenceV1 => InferenceClass::Preference,
        }
    }

    pub fn rule_id(self) -> &'static str {
        match self {
            Self::BaselineDescriptiveV1 => "baseline:descriptive:v1",
            Self::BaselinePreferenceV1 => "baseline:preference:v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferenceRequest {
    run_id: InferenceRunId,
    belief_id: BeliefId,
    class: InferenceClass,
    claim: Claim,
    rule_id: String,
    bases: Vec<JudgmentBasis>,
}

impl InferenceRequest {
    pub fn new(
        run_id: InferenceRunId,
        belief_id: BeliefId,
        rule: TrustedInferenceRule,
        claim: Claim,
        bases: Vec<JudgmentBasis>,
    ) -> Result<Self, RequestError> {
        let class = rule.class();
        let rule_id = rule.rule_id().to_string();
        if bases.is_empty() {
            return Err(RequestError::NoJudgmentBases);
        }

        let mut judgment_ids = BTreeSet::new();
        let mut evidence_values = BTreeMap::new();
        for basis in &bases {
            // Bases are public builder values; callers can bypass or mutate their constructor.
            basis.validate()?;
            if basis.judgment.proposition() != &claim.proposition {
                return Err(RequestError::JudgmentPropositionMismatch {
                    judgment: basis.judgment.id().clone(),
                });
            }
            if !judgment_ids.insert(basis.judgment.id().clone()) {
                return Err(RequestError::DuplicateJudgmentId(
                    basis.judgment.id().clone(),
                ));
            }
            for evidence_use in &basis.evidence {
                let evidence = &evidence_use.evidence;
                if let Some(previous) = evidence_values.insert(&evidence.id, evidence) {
                    if previous != evidence {
                        return Err(RequestError::ConflictingEvidence(evidence.id.clone()));
                    }
                }
            }
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

    pub fn run_id(&self) -> &InferenceRunId {
        &self.run_id
    }

    pub fn belief_id(&self) -> &BeliefId {
        &self.belief_id
    }

    pub fn class(&self) -> InferenceClass {
        self.class
    }

    pub fn claim(&self) -> &Claim {
        &self.claim
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    pub fn bases(&self) -> &[JudgmentBasis] {
        &self.bases
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
        let mut evidence_values = BTreeMap::new();
        for basis in &self.bases {
            for evidence_use in &basis.evidence {
                let evidence = &evidence_use.evidence;
                if !policy.allows_evidence_ref(evidence, evidence_use.purpose) {
                    return Err(AuthorizationError::EvidenceDenied {
                        evidence: evidence.id.clone(),
                        class: evidence.class,
                        purpose: evidence_use.purpose,
                    });
                }
                source_scopes.insert((
                    evidence.provenance.source.repository().to_string(),
                    evidence.provenance.source.scope_id().to_string(),
                ));
                evidence_uses.insert((evidence.id.clone(), evidence_use.purpose));
                evidence_values
                    .entry(evidence.id.clone())
                    .or_insert_with(|| evidence.clone());
            }
        }

        if source_scopes.len() > 1 && !policy.allows_cross_source_join() {
            return Err(AuthorizationError::CrossSourceJoinDenied { source_scopes });
        }

        let authorization = AuthorizationReceipt {
            profile: policy.profile(),
            inference_class: self.class,
            source_scopes: source_scopes.clone(),
            evidence_uses,
            cross_source_join: source_scopes.len() > 1,
            inputs: Arc::new(AuthorizedInputs {
                claim: self.claim.clone(),
                evidence: evidence_values,
            }),
        };

        Ok(AuthorizedInferenceRequest {
            inner: self,
            authorization,
        })
    }
}

#[derive(Debug, PartialEq)]
struct AuthorizedInputs {
    claim: Claim,
    evidence: BTreeMap<EvidenceId, EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorizationReceipt {
    profile: AuthorizationProfile,
    inference_class: InferenceClass,
    source_scopes: BTreeSet<(String, String)>,
    evidence_uses: BTreeSet<(EvidenceId, EvidencePurpose)>,
    cross_source_join: bool,
    // One immutable snapshot per authorization, shared by results and explanations.
    inputs: Arc<AuthorizedInputs>,
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

    /// Exact target claim, including proposition and origin, supplied for authorization.
    pub fn authorized_claim(&self) -> &Claim {
        &self.inputs.claim
    }

    /// Canonical evidence values authorized by this receipt, not just their identifiers.
    /// Stores must compare selected dependencies with these values before writing a belief.
    pub fn authorized_evidence(&self) -> &BTreeMap<EvidenceId, EvidenceRef> {
        &self.inputs.evidence
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
    selected_judgment_values: BTreeMap<JudgmentId, Judgment>,
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

        if belief.id() != &expected.belief_id || belief.claim() != &expected.claim.id {
            return Err(InferenceError::InconsistentResult(
                "belief does not match authorized request".into(),
            ));
        }
        if belief.inference_run() != &expected.run_id {
            return Err(InferenceError::InconsistentResult(
                "belief inference run does not match authorized request".into(),
            ));
        }
        if derivation.belief() != belief.id()
            || derivation.inference_run() != &expected.run_id
            || derivation.rule_id() != expected.rule_id
        {
            return Err(InferenceError::InconsistentResult(
                "derivation does not match authorized request".into(),
            ));
        }
        if derivation.judgments() != &selected_judgments {
            return Err(InferenceError::InconsistentResult(
                "derivation judgments do not match selected judgments".into(),
            ));
        }

        let available_judgments = expected
            .bases
            .iter()
            .map(|basis| basis.judgment.id().clone())
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
            .filter(|basis| selected_judgments.contains(basis.judgment.id()))
            .flat_map(|basis| basis.evidence.iter())
            .map(|evidence_use| evidence_use.evidence.id.clone())
            .collect::<BTreeSet<_>>();
        if derivation.evidence() != &expected_evidence {
            return Err(InferenceError::InconsistentResult(
                "derivation evidence does not match selected authorized judgments".into(),
            ));
        }

        let selected_judgment_values = expected
            .bases
            .iter()
            .filter(|basis| selected_judgments.contains(basis.judgment.id()))
            .map(|basis| (basis.judgment.id().clone(), basis.judgment.clone()))
            .collect::<BTreeMap<_, _>>();

        Ok(Self {
            belief,
            derivation,
            selected_judgments,
            selected_judgment_values,
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

    pub fn selected_judgment_values(&self) -> &BTreeMap<JudgmentId, Judgment> {
        &self.selected_judgment_values
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
        BTreeMap<JudgmentId, Judgment>,
        BTreeSet<JudgmentId>,
        AuthorizationReceipt,
    ) {
        (
            self.belief,
            self.derivation,
            self.selected_judgments,
            self.selected_judgment_values,
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
    JudgmentPropositionMismatch {
        judgment: JudgmentId,
    },
    DuplicateJudgmentId(JudgmentId),
    ConflictingEvidence(EvidenceId),
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
            Self::JudgmentPropositionMismatch { judgment } => write!(
                f,
                "judgment {judgment} does not assess the target claim proposition"
            ),
            Self::DuplicateJudgmentId(judgment) => {
                write!(f, "inference request contains duplicate judgment id {judgment}")
            }
            Self::ConflictingEvidence(evidence) => {
                write!(f, "inference request contains conflicting values for evidence {evidence}")
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
            TrustedInferenceRule::BaselineDescriptiveV1,
            claim(),
            bases,
        )
        .unwrap()
    }

    #[test]
    fn authorization_snapshot_is_deduplicated_and_shared_across_receipt_clones() {
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        for count in [1, 16, 256] {
            let shared = evidence("transcript:1", EvidenceClass::Transcript, "video:1");
            let bases = (0..count)
                .map(|index| basis(&format!("judgment:{index}"), shared.clone()))
                .collect();
            let authorized = request(bases).authorize(&policy).unwrap();
            let receipt = authorized.authorization();
            let cloned = receipt.clone();
            assert_eq!(receipt.authorized_claim(), &claim());
            assert_eq!(receipt.authorized_evidence().len(), 1);
            assert_eq!(receipt.authorized_evidence().get(&shared.id), Some(&shared));
            assert!(Arc::ptr_eq(&receipt.inputs, &cloned.inputs));
        }
    }

    #[test]
    fn request_rejects_judgments_about_an_unrelated_proposition() {
        let evidence_ref = evidence("transcript:1", EvidenceClass::Transcript, "video:1");
        let evidence_id = evidence_ref.id.clone();
        let judgment = Judgment::new(
            JudgmentId::new("judgment:other").unwrap(),
            Proposition::new(
                EntityId::new("person:alice").unwrap(),
                Predicate::new("uses").unwrap(),
                ObjectValue::text("Linux").unwrap(),
            ),
            JudgmentOutcome::Supports,
            Score::new(0.8, ScoreSemantics::ModelConfidence).unwrap(),
            [evidence_id],
            JudgmentSpecRef::new("fixture", "v1").unwrap(),
            "fixture-model:1",
        )
        .unwrap();
        let basis = JudgmentBasis::new(
            judgment,
            EvidenceFamilyId::new("correlation:other").unwrap(),
            vec![EvidenceUse::new(
                evidence_ref,
                EvidencePurpose::Corroboration,
            )],
        )
        .unwrap();

        assert!(matches!(
            InferenceRequest::new(
                InferenceRunId::new("run:1").unwrap(),
                BeliefId::new("belief:1").unwrap(),
                TrustedInferenceRule::BaselineDescriptiveV1,
                claim(),
                vec![basis],
            ),
            Err(RequestError::JudgmentPropositionMismatch { .. })
        ));
    }

    #[test]
    fn request_rejects_duplicate_judgment_ids_across_correlation_groups() {
        let first = basis(
            "judgment:duplicate",
            evidence("transcript:1", EvidenceClass::Transcript, "video:1"),
        );
        let mut second = basis(
            "judgment:duplicate",
            evidence("transcript:2", EvidenceClass::Transcript, "video:1"),
        );
        second.correlation_group = EvidenceFamilyId::new("correlation:other").unwrap();

        assert!(matches!(
            InferenceRequest::new(
                InferenceRunId::new("run:1").unwrap(),
                BeliefId::new("belief:1").unwrap(),
                TrustedInferenceRule::BaselineDescriptiveV1,
                claim(),
                vec![first, second],
            ),
            Err(RequestError::DuplicateJudgmentId(_))
        ));
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
