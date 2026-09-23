use belief_core::{
    BeliefId, Claim, ClaimId, EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId,
    EvidencePurpose, EvidenceRef, InferenceRunId, Judgment, JudgmentId, JudgmentOutcome,
    JudgmentSpecRef, ObjectValue, Predicate, ProducerRef, Proposition, Provenance, Score,
    ScoreSemantics, SourceRef,
};
use belief_policy::PolicyConfig;
use belief_store::{InMemoryBeliefStore, StoreError};
use inference_baseline::BaselineInferenceEngine;
use inference_core::{
    EvidenceUse, InferenceEngine, InferenceRequest, InferenceResult, JudgmentBasis, RequestError,
    TrustedInferenceRule,
};

fn evidence(id: &str) -> EvidenceRef {
    EvidenceRef::new(
        EvidenceId::new(id).unwrap(),
        Some(EntityId::new("document:1").unwrap()),
        EvidenceClass::Transcript,
        EvidenceFamilyId::new(format!("family:{id}")).unwrap(),
        None,
        Provenance::new(
            SourceRef::new("corpus", "source:1", id, "sha256:source-v1").unwrap(),
            ProducerRef::new("fixture", "git:producer-v1", None, None).unwrap(),
            [],
        ),
    )
}

fn proposition() -> Proposition {
    Proposition::new(
        EntityId::new("document:1").unwrap(),
        Predicate::new("mentions_rust").unwrap(),
        ObjectValue::Boolean(true),
    )
}

fn judgment(id: &str, evidence: &EvidenceRef) -> Judgment {
    Judgment::new(
        JudgmentId::new(id).unwrap(),
        proposition(),
        JudgmentOutcome::Supports,
        Score::new(0.8, ScoreSemantics::ModelConfidence).unwrap(),
        [evidence.id.clone()],
        JudgmentSpecRef::new("fixture", "v1").unwrap(),
        "fixture-model:v1",
    )
    .unwrap()
}

fn claim(judgment: &Judgment) -> Claim {
    Claim::from_judgment(
        ClaimId::new("claim:1").unwrap(),
        proposition(),
        judgment.id().clone(),
    )
}

fn basis(judgment: Judgment, evidence: EvidenceRef) -> JudgmentBasis {
    JudgmentBasis::new(
        judgment,
        EvidenceFamilyId::new("correlation:1").unwrap(),
        vec![EvidenceUse::new(evidence, EvidencePurpose::Corroboration)],
    )
    .unwrap()
}

fn request(claim: Claim, bases: Vec<JudgmentBasis>) -> Result<InferenceRequest, RequestError> {
    InferenceRequest::new(
        InferenceRunId::new("run:1").unwrap(),
        BeliefId::new("belief:1").unwrap(),
        TrustedInferenceRule::BaselineDescriptiveV1,
        claim,
        bases,
    )
}

fn infer(claim: Claim, bases: Vec<JudgmentBasis>) -> InferenceResult {
    let policy =
        PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
    let authorized = request(claim, bases).unwrap().authorize(&policy).unwrap();
    BaselineInferenceEngine.infer(&authorized).unwrap()
}

fn seed_store(evidence: EvidenceRef, judgment: Judgment, claim: Claim) -> InMemoryBeliefStore {
    let mut store = InMemoryBeliefStore::default();
    store.insert_evidence(self::evidence("parent:1")).unwrap();
    store.insert_evidence(evidence).unwrap();
    store.insert_judgment(judgment).unwrap();
    store.insert_claim(claim).unwrap();
    store
}

fn assert_rejected_atomically(store: &mut InMemoryBeliefStore, result: InferenceResult) {
    let belief_id = result.belief().id().clone();
    let claim_id = result.belief().claim().clone();
    assert!(matches!(
        store.insert_inference_result(result),
        Err(StoreError::InconsistentResult(_))
    ));
    assert!(store.belief(&belief_id).is_none());
    assert!(store.explain_belief(&belief_id).is_err());
    assert!(store.claim(&claim_id).unwrap().validity.is_active());
}

#[test]
fn request_revalidates_mutated_basis_evidence() {
    for replace_instead_of_remove in [false, true] {
        let evidence = evidence("evidence:1");
        let judgment = judgment("judgment:1", &evidence);
        let claim = claim(&judgment);
        let mut basis = basis(judgment, evidence);
        if replace_instead_of_remove {
            basis.evidence[0].evidence = self::evidence("unrelated:1");
        } else {
            basis.evidence.clear();
        }
        assert!(request(claim, vec![basis]).is_err());
    }
}

#[test]
fn request_rejects_conflicting_same_id_evidence_across_bases() {
    let original = evidence("evidence:1");
    let mut changed = original.clone();
    changed.provenance.source =
        SourceRef::new("corpus", "source:1", "evidence:1", "sha256:source-v2").unwrap();
    let first = judgment("judgment:1", &original);
    let second = judgment("judgment:2", &changed);
    let claim = claim(&first);
    assert!(request(claim, vec![basis(first, original), basis(second, changed)]).is_err());
}

#[test]
fn request_rejects_conflicting_same_id_evidence_within_one_basis() {
    let original = evidence("evidence:1");
    let judgment = judgment("judgment:1", &original);
    let claim = claim(&judgment);
    let mut basis = basis(judgment, original.clone());
    let mut changed = original;
    changed.class = EvidenceClass::OcrText;
    basis
        .evidence
        .push(EvidenceUse::new(changed, EvidencePurpose::Corroboration));
    assert!(request(claim, vec![basis]).is_err());
}

#[test]
fn identical_shared_evidence_remains_valid() {
    let evidence = evidence("evidence:1");
    let first = judgment("judgment:1", &evidence);
    let second = judgment("judgment:2", &evidence);
    let claim = claim(&first);
    let result = infer(
        claim.clone(),
        vec![
            basis(first.clone(), evidence.clone()),
            basis(second, evidence.clone()),
        ],
    );
    let mut store = seed_store(evidence, first, claim);
    // Correlated, ignored judgment:2 need not be stored as a selected dependency.
    store.insert_inference_result(result).unwrap();
    let explanation = store
        .explain_belief(&BeliefId::new("belief:1").unwrap())
        .unwrap();
    assert_eq!(explanation.evidence.len(), 1);
    assert_eq!(explanation.judgments.len(), 1);
}

#[test]
fn store_rejects_same_id_evidence_content_and_provenance_drift() {
    for field in [
        "source_revision",
        "source_scope",
        "source_record",
        "producer",
        "class",
        "family",
        "subject",
        "score",
        "parents",
    ] {
        let original = evidence("evidence:1");
        let judgment = judgment("judgment:1", &original);
        let claim = claim(&judgment);
        let result = infer(
            claim.clone(),
            vec![basis(judgment.clone(), original.clone())],
        );
        let mut changed = original;
        match field {
            "source_revision" => {
                changed.provenance.source =
                    SourceRef::new("corpus", "source:1", "evidence:1", "sha256:source-v2")
                        .unwrap();
            }
            "source_scope" => {
                changed.provenance.source =
                    SourceRef::new("corpus", "source:2", "evidence:1", "sha256:source-v1")
                        .unwrap();
            }
            "source_record" => {
                changed.provenance.source =
                    SourceRef::new("corpus", "source:1", "other-record", "sha256:source-v1")
                        .unwrap();
            }
            "producer" => {
                changed.provenance.producer =
                    ProducerRef::new("fixture", "git:producer-v2", None, None).unwrap();
            }
            "class" => changed.class = EvidenceClass::FaceTrackReference,
            "family" => changed.family = EvidenceFamilyId::new("different-family").unwrap(),
            "subject" => changed.subject = Some(EntityId::new("document:2").unwrap()),
            "score" => {
                changed.score = Some(Score::new(0.2, ScoreSemantics::ModelConfidence).unwrap());
            }
            "parents" => {
                changed
                    .provenance
                    .parent_evidence
                    .insert(EvidenceId::new("parent:1").unwrap());
            }
            _ => unreachable!(),
        }
        let mut store = seed_store(changed.clone(), judgment, claim);
        assert_rejected_atomically(&mut store, result);
        assert_eq!(&store.evidence(&changed.id).unwrap().value, &changed, "{field}");
    }
}

#[test]
fn store_rejects_same_id_claim_proposition_drift() {
    let evidence = evidence("evidence:1");
    let judgment = judgment("judgment:1", &evidence);
    let original = claim(&judgment);
    let result = infer(
        original.clone(),
        vec![basis(judgment.clone(), evidence.clone())],
    );
    let mut changed = original;
    changed.proposition.object = ObjectValue::Boolean(false);
    let mut store = seed_store(evidence, judgment, changed);
    assert_rejected_atomically(&mut store, result);
}

#[test]
fn store_rejects_same_id_claim_origin_drift() {
    let evidence = evidence("evidence:1");
    let judgment = judgment("judgment:1", &evidence);
    let original = claim(&judgment);
    let result = infer(
        original.clone(),
        vec![basis(judgment.clone(), evidence.clone())],
    );
    let changed = Claim::from_user_assertion(
        original.id,
        original.proposition,
        evidence.id.clone(),
    );
    let mut store = seed_store(evidence, judgment, changed);
    assert_rejected_atomically(&mut store, result);
}

#[test]
fn unchanged_authorized_inputs_remain_explainable_after_revocation() {
    let evidence = evidence("evidence:1");
    let judgment = judgment("judgment:1", &evidence);
    let claim = claim(&judgment);
    let result = infer(
        claim.clone(),
        vec![basis(judgment.clone(), evidence.clone())],
    );
    let mut store = seed_store(evidence.clone(), judgment, claim.clone());
    store.insert_inference_result(result).unwrap();
    store.revoke_evidence(&evidence.id, "source withdrawn").unwrap();
    let explanation = store
        .explain_belief(&BeliefId::new("belief:1").unwrap())
        .unwrap();
    assert_eq!(explanation.claim.value, claim);
    assert_eq!(explanation.evidence[0].value, evidence);
    assert!(!explanation.belief.validity.is_active());
}
