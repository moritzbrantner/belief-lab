use belief_cli::attribution::{compare, ComparisonInput};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const EVIDENCE: &str = include_str!("../../../examples/person-attributions/evidence.json");
const ATTRIBUTIONS: &str = include_str!("../../../examples/person-attributions/attributions.json");

fn run(evidence: &str, attribution: &str, profile: &str) -> Result<Value, String> {
    let report = compare(&ComparisonInput {
        evidence_json: evidence,
        attributions_json: attribution,
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), profile.into())]),
    })
    .map_err(|error| error.to_string())?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

fn edit_attributions(change: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(ATTRIBUTIONS).unwrap();
    change(&mut value);
    serde_json::to_string(&value).unwrap()
}

#[test]
fn two_people_can_disagree_without_an_objective_truth_score() {
    let report = run(EVIDENCE, ATTRIBUTIONS, "semantic_research").unwrap();
    assert_eq!(report["report"], "person_belief_comparison");
    assert_eq!(report["people"][0]["stance"], "endorses");
    assert_eq!(report["people"][1]["stance"], "disputes");
    assert_eq!(report["people"][0]["confidence_person"]["value"], 0.91);
    assert_eq!(report["people"][1]["confidence_person"]["value"], 0.73);
    assert_eq!(
        report["people"][0]["observations"][1]["confidence_observation"]["value"],
        0.98
    );
    assert_eq!(
        report["people"][0]["observations"][1]["conviction"]["value"],
        0.3
    );
    assert_eq!(
        report["people"][0]["observations"][1]["conviction"]["basis"],
        "self_reported"
    );
    assert_eq!(
        report["people"][1]["observations"][0]["conviction"]["basis"],
        "inferred"
    );
    assert!(report.get("soft_truth").is_none());
    assert!(report.get("truth").is_none());
    assert_eq!(
        report["people"][0]["observations"][1]["evidence"]["source_revision"],
        "sha256:blog-post-v1"
    );
    assert_eq!(
        report["people"][1]["observations"][1]["evidence"]["producer_name"],
        "sample-excerpt-curation"
    );
    assert_eq!(
        report["people"][0]["observations"][1]["evidence"]["authorized_purpose"],
        "direct_support"
    );
    assert_eq!(
        serde_json::to_string(&report).unwrap(),
        serde_json::to_string(&run(EVIDENCE, ATTRIBUTIONS, "semantic_research").unwrap()).unwrap()
    );
}

#[test]
fn quotations_do_not_become_endorsements() {
    let bad = edit_attributions(|value| {
        value["observations"][1]["stance"] = json!("endorses");
    });
    assert!(run(EVIDENCE, &bad, "semantic_research")
        .unwrap_err()
        .contains("quotation/criticism"));

    let report = run(EVIDENCE, ATTRIBUTIONS, "semantic_research").unwrap();
    assert_eq!(
        report["people"][0]["observations"][0]["stance"],
        "undetermined"
    );
    assert!(report["people"][0]["observations"][0]["confidence_observation"].is_null());
    assert!(report["people"][0]["observations"][0]["conviction"].is_null());
}

#[test]
fn missing_and_conflicting_evidence_do_not_generate_numeric_assessments() {
    let uncertain = edit_attributions(|value| {
        value["observations"][0]
            .as_object_mut()
            .unwrap()
            .remove("confidenceObservation");
        value["observations"][0]
            .as_object_mut()
            .unwrap()
            .remove("conviction");
        value["assessments"] = json!([]);
    });
    let report = run(EVIDENCE, &uncertain, "semantic_research").unwrap();
    assert!(report["people"][0]["confidence_person"].is_null());
    assert!(report["people"][0]["observations"][1]["confidence_observation"].is_null());

    let conflict = edit_attributions(|value| {
        value["observations"][1]["kind"] = json!("inferred_belief");
        value["observations"][1]["stance"] = json!("disputes");
        value["observations"][1]["confidenceObservation"] =
            json!({"value":0.4,"assessedBy":"scripted_example"});
        value["assessments"] = json!([]);
    });
    let report = run(EVIDENCE, &conflict, "semantic_research").unwrap();
    assert_eq!(report["people"][0]["has_conflicting_positions"], true);
    assert!(report["people"][0]["confidence_person"].is_null());

    let unresolved = edit_attributions(|value| {
        value["observations"][1]["kind"] = json!("inferred_belief");
        value["observations"][1]["stance"] = json!("disputes");
        value["observations"][1]["confidenceObservation"] =
            json!({"value":0.4,"assessedBy":"scripted_example"});
    });
    assert!(run(EVIDENCE, &unresolved, "semantic_research")
        .unwrap_err()
        .contains("reviewedConflicts"));
}

#[test]
fn rejects_source_identity_substitution_and_policy_bypass() {
    assert!(run(EVIDENCE, ATTRIBUTIONS, "observe_only").is_err());

    let wrong_subject = edit_attributions(|value| {
        value["people"][0]["id"] = json!("researcher-b");
    });
    assert!(run(EVIDENCE, &wrong_subject, "semantic_research").is_err());

    let injected_score = edit_attributions(|value| {
        value["assessments"][0]["confidencePerson"]["semantics"] = json!("soft_truth");
    });
    assert!(run(EVIDENCE, &injected_score, "semantic_research").is_err());

    let wrong_conviction = edit_attributions(|value| {
        value["observations"][2]["conviction"] = json!({
            "value":0.9,"basis":"self_reported","assessedBy":"scripted_example"
        });
    });
    assert!(run(EVIDENCE, &wrong_conviction, "semantic_research").is_err());
}

#[test]
fn persons_with_same_display_name_remain_distinct_source_local_records() {
    let same_names = edit_attributions(|value| {
        value["people"][1]["displayName"] = json!("Blogger A");
    });
    let report = run(EVIDENCE, &same_names, "semantic_research").unwrap();
    assert_eq!(
        report["people"][0]["display_name"],
        report["people"][1]["display_name"]
    );
    assert_ne!(report["people"][0]["id"], report["people"][1]["id"]);
    assert_ne!(
        report["people"][0]["scope_id"],
        report["people"][1]["scope_id"]
    );
}

#[test]
fn repository_and_scope_are_required_for_source_local_person_binding() {
    // Reusing a corpus-local subject token and scope in a different repository is
    // not an authorized cross-source person link.
    let mut evidence: Value = serde_json::from_str(EVIDENCE).unwrap();
    let mut annotations: Value = serde_json::from_str(ATTRIBUTIONS).unwrap();
    let mut other = evidence["evidence"][0].clone();
    other["id"] = json!("e-other-corpus");
    other["source"]["repository"] = json!("different-corpus");
    evidence["evidence"].as_array_mut().unwrap().push(other);
    annotations["observations"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"from-another-corpus",
            "personId":"blogger-a",
            "evidenceId":"e-other-corpus",
            "kind":"explicit_endorsement",
            "stance":"disputes",
            "passage":"I dispute the proposition.",
            "span":{"start":0,"end":26},
            "confidenceObservation":{"value":0.8,"assessedBy":"scripted_example"}
        }));
    let e = serde_json::to_string(&evidence).unwrap();
    let a = serde_json::to_string(&annotations).unwrap();
    assert!(run(&e, &a, "semantic_research")
        .unwrap_err()
        .contains("not bound to source-local person"));

    annotations["observations"][4]["personId"] = json!("blogger-other");
    annotations["people"].as_array_mut().unwrap().push(json!({
        "id":"blogger-other",
        "subjectId":"blogger-a",
        "sourceRepository":"different-corpus",
        "scopeId":"blog-post-1",
        "displayName":"Blogger A"
    }));
    let report = run(
        &e,
        &serde_json::to_string(&annotations).unwrap(),
        "semantic_research",
    )
    .unwrap();
    assert_eq!(report["people"].as_array().unwrap().len(), 3);
    assert_eq!(report["people"][0]["source_repository"], "sample-corpus");
    assert_eq!(report["people"][1]["source_repository"], "different-corpus");
    assert_eq!(report["people"][0]["subject_id"], "blogger-a");
    assert_eq!(report["people"][1]["subject_id"], "blogger-a");
    assert_ne!(report["people"][0]["id"], report["people"][1]["id"]);

    let misbound = edit_attributions(|value| {
        value["people"][0]["sourceRepository"] = json!("different-corpus");
    });
    assert!(run(EVIDENCE, &misbound, "semantic_research").is_err());
}
