//! Decisions 1.4.25, 1.4.27, 1.4.29 and 1.4.31: four stance distinctions, reviewable conflict,
//! dated history and an explicitly uncertain current stance.
use belief_cli::attribution::{compare, ComparisonInput};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const EVIDENCE: &str = include_str!("../../../examples/person-attributions/evidence.json");
const ATTRIBUTIONS: &str = include_str!("../../../examples/person-attributions/attributions.json");

fn run(document: &Value) -> Result<Value, String> {
    let report = compare(&ComparisonInput {
        evidence_json: EVIDENCE,
        attributions_json: &document.to_string(),
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), "semantic_research".into())]),
    })
    .map_err(|error| error.to_string())?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

fn example() -> Value {
    serde_json::from_str(ATTRIBUTIONS).unwrap()
}

fn person<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|person| person["id"] == id)
        .unwrap()
}

fn observation<'a>(document: &'a mut Value, id: &str) -> &'a mut Value {
    document["observations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|observation| observation["id"] == id)
        .unwrap()
}

fn without_assessments(mut document: Value) -> Value {
    document["assessments"] = json!([]);
    document
}

#[test]
fn a_quotation_alone_is_no_stance_evidence_not_indecision() {
    let mut document = without_assessments(example());
    document["observations"]
        .as_array_mut()
        .unwrap()
        .retain(|item| item["id"] != "a-stated");
    let report = run(&document).unwrap();
    let blogger = person(&report, "blogger-a");
    assert_eq!(blogger["assessment_status"], "no_stance_evidence");
    assert!(blogger["stance"].is_null());
    assert_eq!(blogger["observations"][0]["stance"], "no_stance");
    assert_eq!(blogger["current_stance"]["status"], "unknown");
}

#[test]
fn explicit_suspension_needs_an_expressed_stance_and_is_distinct() {
    let mut document = example();
    *observation(&mut document, "a-stated") = json!({
        "id": "a-stated", "personId": "blogger-a", "evidenceId": "e-blog",
        "kind": "explicit_endorsement", "stance": "suspends_judgment",
        "passage": "I honestly don't know yet whether it lasts into 2027.",
        "span": {"start": 0, "end": 53},
        "confidenceObservation": {"value": 0.9, "assessedBy": "scripted_example"}
    });
    document["assessments"][0] = json!({
        "personId": "blogger-a", "stance": "suspends_judgment",
        "confidencePerson": {"value": 0.8, "assessedBy": "scripted_example"},
        "observationIds": ["a-stated"]
    });
    let report = run(&document).unwrap();
    let blogger = person(&report, "blogger-a");
    assert_eq!(blogger["assessment_status"], "assessed");
    assert_eq!(blogger["stance"], "suspends_judgment");
    assert!(!blogger["has_conflicting_positions"].as_bool().unwrap());

    let mut inferred = document.clone();
    observation(&mut inferred, "a-stated")["kind"] = json!("inferred_belief");
    assert!(run(&inferred).unwrap_err().contains("suspending judgment"));

    let mut with_conviction = document;
    observation(&mut with_conviction, "a-stated")["conviction"] =
        json!({"value": 0.2, "basis": "self_reported", "assessedBy": "scripted_example"});
    assert!(run(&with_conviction)
        .unwrap_err()
        .contains("suspending judgment"));
}

#[test]
fn opposing_stances_in_overlapping_time_stay_conflicted_without_a_winner() {
    let mut document = without_assessments(example());
    let reported = observation(&mut document, "b-reported");
    reported["stance"] = json!("endorses");
    reported["expressedAt"] = json!("2024-05");
    observation(&mut document, "b-inferred")["expressedAt"] = json!("2024");
    let report = run(&document).unwrap();
    let researcher = person(&report, "researcher-b");
    assert_eq!(researcher["assessment_status"], "conflicted");
    assert!(researcher["stance"].is_null());
    assert_eq!(
        researcher["conflicting_observation_ids"],
        json!(["b-inferred", "b-reported"])
    );
    assert!(researcher["last_documented"].is_null());

    document["assessments"] = json!([{
        "personId": "researcher-b", "stance": "endorses",
        "confidencePerson": {"value": 0.6, "assessedBy": "analyst"},
        "observationIds": ["b-reported"]
    }]);
    assert!(run(&document).unwrap_err().contains("reviewedConflicts"));
    document["assessments"][0]["reviewedConflicts"] = json!(true);
    let reconciled = run(&document).unwrap();
    let researcher = person(&reconciled, "researcher-b");
    assert_eq!(researcher["assessment_status"], "assessed");
    assert!(researcher["has_conflicting_positions"].as_bool().unwrap());
    assert_eq!(
        researcher["supporting_observation_ids"],
        json!(["b-reported"])
    );
}

#[test]
fn opposing_stances_at_distinct_dates_are_history_not_a_settled_change_of_mind() {
    let mut document = without_assessments(example());
    let reported = observation(&mut document, "b-reported");
    reported["stance"] = json!("endorses");
    reported["expressedAt"] = json!("2019-04-02");
    observation(&mut document, "b-inferred")["expressedAt"] = json!("2025-11");
    let report = run(&document).unwrap();
    let researcher = person(&report, "researcher-b");
    assert_eq!(researcher["assessment_status"], "differs_over_time");
    assert_eq!(researcher["conflicting_observation_ids"], json!([]));
    assert_eq!(researcher["last_documented"]["stance"], "endorses_negation");
    assert_eq!(
        researcher["last_documented"]["observation_ids"],
        json!(["b-inferred"])
    );
    assert_eq!(researcher["current_stance"]["status"], "unknown");
}

#[test]
fn old_documented_stance_sits_beside_an_uncertain_current_estimate() {
    let report = run(&example()).unwrap();
    let researcher = person(&report, "researcher-b");
    assert_eq!(researcher["last_documented"]["stance"], "endorses_negation");
    assert_eq!(
        researcher["last_documented"]["expressed_at"],
        json!(["2024"])
    );
    assert_eq!(
        researcher["last_documented"]["undated_observation_ids"],
        json!(["b-inferred"])
    );
    assert_eq!(researcher["current_stance"]["status"], "unknown");

    let mut document = example();
    document["assessments"].as_array_mut().unwrap().push(json!({
        "personId": "researcher-b", "timeScope": "current", "stance": "endorses_negation",
        "observationIds": ["b-reported"], "asOf": "2026-10"
    }));
    assert!(run(&document).unwrap_err().contains("rationale"));
    document["assessments"][2]["rationale"] =
        json!("Only a 2024 interview; no newer statement, so this is a tentative estimate.");
    let estimated = run(&document).unwrap();
    let researcher = person(&estimated, "researcher-b");
    assert_eq!(researcher["current_stance"]["status"], "estimated");
    assert_eq!(researcher["current_stance"]["as_of"], "2026-10");
    assert!(researcher["current_stance"]["confidence_person"].is_null());
    // The documented assessment is unchanged by the estimate.
    assert_eq!(researcher["stance"], "endorses_negation");
    assert_eq!(researcher["confidence_person"]["value"], 0.73);
}

#[test]
fn dates_are_validated_and_never_reordered() {
    let mut document = example();
    observation(&mut document, "a-stated")["expressedAt"] = json!("2026-02-30");
    assert!(run(&document).unwrap_err().contains("expressedAt"));

    let mut document = example();
    let stated = observation(&mut document, "a-stated");
    stated["expressedAt"] = json!("2026-05");
    stated["publishedAt"] = json!("2026-03-14");
    assert!(run(&document)
        .unwrap_err()
        .contains("published before it was expressed"));
}

#[test]
fn version_one_names_remain_readable() {
    let mut document = example();
    observation(&mut document, "b-reported")["stance"] = json!("disputes");
    observation(&mut document, "b-inferred")["stance"] = json!("disputes");
    observation(&mut document, "a-quotation")["stance"] = json!("undetermined");
    document["assessments"][1]["stance"] = json!("disputes");
    let report = run(&document).unwrap();
    assert_eq!(report, run(&example()).unwrap());
}
