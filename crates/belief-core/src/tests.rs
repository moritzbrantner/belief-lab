use crate::*;

fn fixture() -> BeliefEvidenceBundleV1 {
    serde_json::from_str(include_str!("../../../fixtures/multimodal-person.json")).unwrap()
}

#[test]
fn fixture_validates_and_dogfoods_corpus_ids() {
    let validated = validate_bundle(fixture()).unwrap();
    assert_eq!(validated.source_count(), 1);
    assert_eq!(validated.span_count(), 1);
    assert_eq!(validated.video_count(), 1);
    assert!(validated.evidence_count() >= 10);
}

#[test]
fn baseline_inference_keeps_score_semantics_explicit() {
    let validated = validate_bundle(fixture()).unwrap();
    let claims = infer_baseline(&validated);
    assert_eq!(claims.len(), 1);
    assert_eq!(
        claims[0].support.semantics,
        ScoreSemanticsV1::HeuristicStrength
    );
    assert!((claims[0].support.value - 0.88).abs() < f64::EPSILON);
}

#[test]
fn explanation_reaches_source_and_cross_modal_evidence() {
    let validated = validate_bundle(fixture()).unwrap();
    let claims = infer_baseline(&validated);
    let explanation = explain_claim(&validated, &claims[0]).unwrap();
    let json = serde_json::to_string(&explanation).unwrap();
    assert!(json.contains("span-transcript-1"));
    assert!(json.contains("ocr-track-1"));
    assert!(json.contains("face-track-4"));
    assert!(json.contains("voice-track-2"));
    assert!(json.contains("heuristic_strength"));
}

#[test]
fn unknown_evidence_reference_fails_closed() {
    let mut bundle = fixture();
    bundle.observations[0]
        .evidence_refs
        .push("missing-evidence".to_string());
    let error = validate_bundle(bundle).unwrap_err();
    assert!(matches!(
        error,
        BeliefError::UnknownEvidenceReference { .. }
    ));
}

#[test]
fn duplicate_evidence_ids_fail_closed() {
    let mut bundle = fixture();
    bundle.observations[1].id = bundle.observations[0].id.clone();
    let error = validate_bundle(bundle).unwrap_err();
    assert!(matches!(error, BeliefError::DuplicateEvidence(_)));
}
