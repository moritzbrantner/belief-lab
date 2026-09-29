use evidence_interchange::ValidatedEvidenceBatch;
use serde_json::{json, Value};
#[test]
fn producer_payloads_do_not_become_belief_evidence() {
    let source = include_str!("../../../fixtures/evidence-interchange/youtube-multimodal-v1.json");
    assert!(ValidatedEvidenceBatch::parse_json(source).is_ok());
    for field in [
        "transcript",
        "text",
        "boundingBoxes",
        "embedding",
        "scene",
        "face",
        "audio",
        "entities",
    ] {
        let mut batch: Value = serde_json::from_str(source).unwrap();
        batch["evidence"][0][field] = json!("producer-owned payload");
        assert!(
            ValidatedEvidenceBatch::parse_json(&batch.to_string()).is_err(),
            "accepted {field}"
        );
    }
    let purpose: Value =
        serde_json::from_str(include_str!("../../../repository-purpose.json")).unwrap();
    assert_eq!(purpose["role"], "downstream_epistemic_system");
    assert_eq!(purpose["interchange"], "belief_evidence_interchange@1");
    assert!(purpose["excludes"]
        .as_array()
        .unwrap()
        .contains(&json!("corpus_persistence")));
}
