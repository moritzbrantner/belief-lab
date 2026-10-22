//! #35 slice 1: portable investigation projects with several evidence batches and full history.
use belief_cli::attribution::{compare, ComparisonInput};
use belief_cli::project::open_project;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const EVIDENCE: &str = include_str!("../../../examples/person-attributions/evidence.json");
const ATTRIBUTIONS: &str = include_str!("../../../examples/person-attributions/attributions.json");

fn policy() -> BTreeMap<String, String> {
    BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), "semantic_research".into())])
}

fn open(project: &Value) -> Result<Value, String> {
    let report =
        open_project(&project.to_string(), &policy()).map_err(|error| error.to_string())?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

fn project(id: &str, batches: Vec<Value>, annotations: Value) -> Value {
    json!({
        "schema": "belief_investigation_project", "schemaVersion": 1,
        "investigation": {"id": id, "name": format!("Research {id}"), "createdAt": "2026-10-22T08:00:00Z"},
        "evidenceBatches": batches.into_iter()
            .map(|batch| json!({"importedAt": "2026-10-22T08:01:00Z", "batch": batch}))
            .collect::<Vec<_>>(),
        "annotations": annotations
    })
}

fn evidence() -> Value {
    serde_json::from_str(EVIDENCE).unwrap()
}

/// A later producer batch with one new interview record for Researcher B.
fn second_batch() -> Value {
    let mut batch = evidence();
    batch["revision"] = json!("sha256:follow-up-interview-v1");
    let mut record = batch["evidence"][2].clone();
    record["id"] = json!("e-follow-up");
    record["source"]["recordId"] = json!("paragraph-9");
    batch["evidence"] = json!([record]);
    batch
}

#[test]
fn a_project_replays_to_the_same_comparison_as_its_inputs() {
    let annotations: Value = serde_json::from_str(ATTRIBUTIONS).unwrap();
    let report = open(&project("a", vec![evidence()], annotations.clone())).unwrap();
    let direct = compare(&ComparisonInput {
        evidence_json: EVIDENCE,
        attributions_json: ATTRIBUTIONS,
        policy: policy(),
    })
    .unwrap();
    assert_eq!(report["comparison"], serde_json::to_value(direct).unwrap());
    assert_eq!(report["investigation"]["name"], "Research a");
    assert_eq!(report["batches"][0]["inserted"], 4);
}

#[test]
fn a_new_investigation_may_start_without_annotations() {
    let report = open(&project("b", vec![], Value::Null)).unwrap();
    assert!(report["comparison"].is_null());
    assert_eq!(report["batches"], json!([]));
}

#[test]
fn adding_a_batch_continues_the_investigation_without_merging_receipts() {
    let mut annotations: Value = serde_json::from_str(ATTRIBUTIONS).unwrap();
    annotations["observations"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "b-follow-up", "personId": "researcher-b", "evidenceId": "e-follow-up",
            "kind": "explicit_endorsement", "stance": "endorses_negation",
            "passage": "I still doubt the 2027 claim.", "span": {"start": 0, "end": 29},
            "expressedAt": "2026-09"
        }));
    let report = open(&project("a", vec![evidence(), second_batch()], annotations)).unwrap();
    assert_eq!(report["batches"].as_array().unwrap().len(), 2);
    assert_eq!(report["batches"][1]["evidence_ids"], json!(["e-follow-up"]));
    let researcher = &report["comparison"]["people"][1];
    assert_eq!(researcher["observations"].as_array().unwrap().len(), 3);
    assert_eq!(
        researcher["last_documented"]["expressed_at"],
        json!(["2026-09"])
    );

    // Re-importing identical records is harmless; changing one under the same id is refused.
    let repeated = open(&project("a", vec![evidence(), evidence()], Value::Null)).unwrap();
    assert_eq!(repeated["batches"][1]["already_present"], 4);
    let mut changed = second_batch();
    changed["evidence"][0]["id"] = json!("e-report");
    let error = open(&project("a", vec![evidence(), changed], Value::Null)).unwrap_err();
    assert!(error.contains("e-report"), "{error}");
}

#[test]
fn tampered_or_invalid_projects_are_refused() {
    let annotations: Value = serde_json::from_str(ATTRIBUTIONS).unwrap();
    let valid = project("a", vec![evidence()], annotations);
    let cases: Vec<(&str, Value)> = vec![
        ("schema/version", {
            let mut item = valid.clone();
            item["schemaVersion"] = json!(2);
            item
        }),
        ("investigation id", {
            let mut item = valid.clone();
            item["investigation"]["id"] = json!("../escape");
            item
        }),
        ("investigation name", {
            let mut item = valid.clone();
            item["investigation"]["name"] = json!(" ");
            item
        }),
        ("unknown imported evidence", {
            let mut item = valid.clone();
            item["evidenceBatches"] = json!([]);
            item
        }),
        ("quotation/criticism", {
            let mut item = valid.clone();
            item["annotations"]["observations"][1]["stance"] = json!("endorses");
            item
        }),
        ("unknown field", {
            let mut item = valid.clone();
            item["trustedApproval"] = json!(true);
            item
        }),
    ];
    for (expected, item) in cases {
        let error = open(&item).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }
    let mut denied = policy();
    denied.insert("BELIEF_POLICY_PROFILE".into(), "default".into());
    assert!(open_project(&valid.to_string(), &denied).is_err());
}
