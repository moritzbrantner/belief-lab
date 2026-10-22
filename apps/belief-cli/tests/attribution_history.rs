//! Independent preimplementation contract for #37's approved revision-history slice.
//! Baseline 866d4cfa8cabeab6089fabdfae38ef67026481c9; investigator-supplied history.
use belief_cli::attribution::{compare, ComparisonInput};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const EVIDENCE: &str = include_str!("../../../examples/person-attributions/evidence.json");
const ANNOTATIONS: &str = include_str!("../../../examples/person-attributions/attributions.json");

fn original() -> Value {
    serde_json::from_str(ANNOTATIONS).unwrap()
}
fn run(document: &Value) -> Result<Value, String> {
    let annotations = document.to_string();
    compare(&ComparisonInput {
        evidence_json: EVIDENCE,
        attributions_json: &annotations,
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), "semantic_research".into())]),
    })
    .map(|report| serde_json::to_value(report).unwrap())
    .map_err(|error| error.to_string())
}
fn history(revisions: Vec<(&str, Value)>) -> Value {
    json!({"schema":"belief_person_attributions", "schemaVersion":2,
        "revisions":revisions.into_iter().map(|(reason,document)|json!({"reason":reason,"document":document})).collect::<Vec<_>>()})
}
fn person<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap()
}
fn unassessed(mut document: Value) -> Value {
    document["assessments"]
        .as_array_mut()
        .unwrap()
        .retain(|a| a["personId"] != "blogger-a");
    document
}

#[test]
fn version_one_remains_an_unchanged_current_comparison() {
    let report = run(&original()).unwrap();
    assert_eq!(person(&report, "blogger-a")["stance"], "endorses");
    assert_eq!(
        person(&report, "blogger-a")["confidence_person"]["value"],
        0.91
    );
    assert_eq!(
        person(&report, "researcher-b")["confidence_person"]["value"],
        0.73
    );
    assert!(report
        .get("history")
        .is_none_or(|h| h.as_array().is_some_and(Vec::is_empty)));
}

#[test]
fn correction_and_withdrawal_preserve_ordered_prior_snapshots_and_exact_passages() {
    let first = original();
    let original_report = run(&first).unwrap();
    let mut corrected = unassessed(first.clone());
    corrected["observations"][0]["id"] = json!("a-corrected");
    corrected["observations"][0]["passage"] = json!("  Corrected investigator annotation.\n");
    corrected["observations"][0]["span"] = json!({"start":17,"end":53});
    let corrected_report = run(&corrected).unwrap();
    let mut withdrawn = corrected.clone();
    withdrawn["observations"]
        .as_array_mut()
        .unwrap()
        .retain(|o| o["id"] != "a-corrected");
    let project = history(vec![
        ("Initial prepared annotations", first),
        ("Correct passage transcription", corrected),
        ("Withdraw unsupported observation", withdrawn),
    ]);
    let report = run(&project).unwrap();
    let snapshots = report["history"]
        .as_array()
        .expect("historical comparisons");
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0]["revision"], 1);
    assert_eq!(snapshots[0]["reason"], "Initial prepared annotations");
    assert_eq!(snapshots[0]["people"], original_report["people"]);
    assert_eq!(snapshots[1]["revision"], 2);
    assert_eq!(snapshots[1]["reason"], "Correct passage transcription");
    assert_eq!(snapshots[1]["people"], corrected_report["people"]);
    assert!(person(&report, "blogger-a")["stance"].is_null());
    assert_eq!(
        person(&report, "blogger-a")["assessment_status"],
        "no_stance_evidence"
    );
    assert!(person(&report, "blogger-a")["confidence_person"].is_null());
    assert!(!person(&report, "blogger-a")["observations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["id"] == "a-corrected"));
    assert_eq!(
        person(&report, "researcher-b"),
        person(&original_report, "researcher-b")
    );
    let replayed: Value =
        serde_json::from_str(&serde_json::to_string_pretty(&project).unwrap()).unwrap();
    assert_eq!(run(&replayed).unwrap(), report);
    for snapshot in snapshots.iter().chain(std::iter::once(&report)) {
        assert!(snapshot.get("soft_truth").is_none());
        assert!(snapshot.get("truth").is_none());
    }
}

#[test]
fn adding_or_changing_observation_cannot_silently_carry_forward_an_assessment() {
    for mutation in ["add", "change"] {
        let first = original();
        let mut next = first.clone();
        if mutation == "add" {
            let mut observation = first["observations"][1].clone();
            observation["id"] = json!("new-quotation");
            next["observations"]
                .as_array_mut()
                .unwrap()
                .push(observation);
        } else {
            next["observations"][0]["confidenceObservation"]["value"] = json!(0.4);
        }
        // Refusal or explicit unknown is safe; retaining the old definite score is not.
        if let Ok(report) = run(&history(vec![
            ("Initial", first),
            ("New evidence interpretation", next),
        ])) {
            assert!(
                person(&report, "blogger-a")["stance"].is_null(),
                "{mutation}"
            );
            assert!(
                person(&report, "blogger-a")["confidence_person"].is_null(),
                "{mutation}"
            );
            assert_eq!(
                person(&report, "researcher-b")["confidence_person"]["value"],
                0.73
            );
        }
    }
}

#[test]
fn explicit_reassessment_is_a_distinct_inspectable_revision() {
    let first = original();
    let mut second = first.clone();
    second["assessments"][0]["confidencePerson"] = json!({"value":0.6,"assessedBy":"analyst"});
    let report = run(&history(vec![
        ("Initial", first),
        ("Reassessed support with unchanged observations", second),
    ]))
    .unwrap();
    assert_eq!(
        person(&report, "blogger-a")["confidence_person"]["value"],
        0.6
    );
    assert_eq!(
        person(&report["history"][0], "blogger-a")["confidence_person"]["value"],
        0.91
    );
}

#[test]
fn malformed_or_unauthorized_history_is_rejected_including_old_snapshots() {
    let valid = history(vec![("Initial", original())]);
    for bad in [
        json!({"schema":"belief_person_attributions","schemaVersion":2,"revisions":[]}),
        json!({"schema":"belief_person_attributions","schemaVersion":2,"revisions":[{"reason":" ","document":original()}]}),
        json!({"schema":"belief_person_attributions","schemaVersion":2,"revisions":[{"reason":"Initial"}]}),
        json!({"schema":"belief_person_attributions","schemaVersion":2,"revisions":[{"reason":"Initial","document":valid}]}),
    ] {
        assert!(run(&bad).is_err(), "{bad}");
    }
    let mut malformed_old = original();
    malformed_old["observations"][0]["evidenceId"] = json!("missing-evidence");
    assert!(run(&history(vec![
        ("Invalid old snapshot", malformed_old),
        ("Current", original())
    ]))
    .is_err());
    let mut wrong_person = original();
    wrong_person["people"][0]["sourceRepository"] = json!("another-repository");
    assert!(run(&history(vec![
        ("Invalid historical source binding", wrong_person),
        ("Current", original())
    ]))
    .is_err());
}

// Implementation-added regression complementing the independent contract above.
#[test]
fn stale_equal_scores_need_an_explicit_new_review_and_corrections_bind_real_ids() {
    let first = original();
    let mut next = first.clone();
    let mut added = first["observations"][1].clone();
    added["id"] = json!("another-quotation");
    next["observations"].as_array_mut().unwrap().push(added);
    let mut log = history(vec![
        ("Initial", first.clone()),
        ("New quotation", next.clone()),
        ("No new review", next),
    ]);
    let report = run(&log).unwrap();
    assert!(person(&report, "blogger-a")["confidence_person"].is_null());
    assert!(person(&report["history"][1], "blogger-a")["confidence_person"].is_null());
    log["revisions"][2]["reassessedPersonIds"] = json!(["blogger-a"]);
    let reviewed = run(&log).unwrap();
    assert_eq!(
        person(&reviewed, "blogger-a")["confidence_person"]["value"],
        0.91
    );
    assert!(person(&reviewed["history"][1], "blogger-a")["confidence_person"].is_null());
    log["revisions"][2]["supersedes"] = json!({"another-quotation":"a-stated"});
    assert!(
        run(&log).is_err(),
        "correction cannot retain the superseded current record or reuse a prior id"
    );
    let mut changed_scope = history(vec![("Initial", first.clone()), ("Retarget", first)]);
    changed_scope["revisions"][1]["document"]["proposition"]["scope"] =
        json!("Different population");
    assert!(run(&changed_scope).is_err());
}
