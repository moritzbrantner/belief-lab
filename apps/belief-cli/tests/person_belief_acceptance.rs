//! Retrospective independent acceptance for the bounded source-local comparison.
//! Authority: Decision Atlas 1.4.1, 1.4.3, 1.4.7 and 1.4.9 at 0ac73bf.
use belief_cli::attribution::{compare, ComparisonInput};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const EVIDENCE: &str = include_str!("../../../examples/person-attributions/evidence.json");
const ANNOTATIONS: &str = include_str!("../../../examples/person-attributions/attributions.json");

fn compare_annotations(annotations: Value) -> Result<Value, String> {
    let annotations = annotations.to_string();
    compare(&ComparisonInput {
        evidence_json: EVIDENCE,
        attributions_json: &annotations,
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), "semantic_research".into())]),
    })
    .map(|report| serde_json::to_value(report).unwrap())
    .map_err(|error| error.to_string())
}

#[test]
fn an_unscored_person_assessment_cannot_assert_a_definite_belief() {
    for stance in ["endorses", "disputes"] {
        let mut input: Value = serde_json::from_str(ANNOTATIONS).unwrap();
        input["assessments"][0]["stance"] = json!(stance);
        input["assessments"][0]
            .as_object_mut()
            .unwrap()
            .remove("confidencePerson");
        // Invalid declarations may be refused or retained as unresolved candidates.
        // The source-level observations must never stand in for person confidence.
        if let Ok(report) = compare_annotations(input) {
            assert!(report["people"][0]["stance"].is_null());
            assert!(report["people"][0]["confidence_person"].is_null());
        }
    }
}

#[test]
fn reviewing_a_conflict_does_not_make_opposition_supporting_evidence() {
    let mut input: Value = serde_json::from_str(ANNOTATIONS).unwrap();
    input["observations"][1]["kind"] = json!("inferred_belief");
    input["observations"][1]["stance"] = json!("disputes");
    input["observations"][1]["confidenceObservation"] = json!({"value":0.6,"assessedBy":"analyst"});
    input["assessments"][0]["reviewedConflicts"] = json!(true);
    input["assessments"][0]["observationIds"] = json!(["a-stated", "a-quotation"]);
    // The analyst may review contrary evidence, but it cannot be described as
    // support for endorsement. Refusal or explicit separation both preserve intent.
    if let Ok(report) = compare_annotations(input) {
        let person = &report["people"][0];
        assert_eq!(person["has_conflicting_positions"], true);
        assert!(person["observations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|observation| {
                observation["id"] == "a-quotation" && observation["stance"] == "disputes"
            }));
        assert!(!person["supporting_observation_ids"]
            .as_array()
            .unwrap()
            .contains(&json!("a-quotation")));
    }
}
