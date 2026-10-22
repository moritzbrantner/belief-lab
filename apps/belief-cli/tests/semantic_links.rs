//! Decision 1.4.33: reviewable semantic relationships between differently worded propositions.
//! A relationship never merges propositions, reverses an implication or attributes a belief.
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

/// The version-1 example as one investigation snapshot with a second proposition Q.
fn snapshot() -> Value {
    let mut document: Value = serde_json::from_str(ATTRIBUTIONS).unwrap();
    for list in ["observations", "assessments"] {
        for item in document[list].as_array_mut().unwrap() {
            item["propositionId"] = json!("effects-2027");
        }
    }
    json!({
        "propositions": [
            document["proposition"].clone(),
            {"id": "effects-2026", "text": "The intervention's effects last through 2026",
             "scope": "Adults who received intervention X in 2026"}
        ],
        "people": document["people"],
        "observations": document["observations"],
        "assessments": document["assessments"],
        "relationships": [implication()]
    })
}

fn implication() -> Value {
    json!({
        "id": "lasts-2027-implies-2026", "kind": "implication",
        "fromPropositionId": "effects-2027", "toPropositionId": "effects-2026",
        "context": "Same adults and intervention; 'through' means continuously",
        "reasoning": "Lasting through 2027 includes lasting through 2026.",
        "suggestedBy": "scripted_example", "review": {"status": "suggested"}
    })
}

fn investigation(revisions: Vec<(&str, Value)>) -> Value {
    json!({
        "schema": "belief_person_attributions", "schemaVersion": 3,
        "revisions": revisions.into_iter()
            .map(|(reason, document)| json!({"reason": reason, "document": document}))
            .collect::<Vec<_>>()
    })
}

fn proposition<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["propositions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["proposition"]["id"] == id)
        .unwrap()
}

#[test]
fn an_accepted_implication_attributes_nothing_about_q() {
    let mut accepted = snapshot();
    accepted["relationships"][0]["review"] = json!({"status": "accepted", "note": "Checked scope"});
    let report = run(&investigation(vec![
        ("Prepared annotations", snapshot()),
        ("Accept the implication", accepted),
    ]))
    .unwrap();
    assert_eq!(report["report"], "person_belief_investigation");

    let single: Value = run(&serde_json::from_str(ATTRIBUTIONS).unwrap()).unwrap();
    assert_eq!(
        proposition(&report, "effects-2027")["people"],
        single["people"]
    );
    for person in proposition(&report, "effects-2026")["people"]
        .as_array()
        .unwrap()
    {
        assert_eq!(person["assessment_status"], "no_stance_evidence");
        assert!(person["stance"].is_null());
        assert!(person["confidence_person"].is_null());
        assert_eq!(person["observations"], json!([]));
    }

    let links = report["relationships"].as_array().unwrap();
    assert_eq!(links.len(), 1, "no reverse Q -> P link is synthesized");
    assert_eq!(links[0]["from_proposition_id"], "effects-2027");
    assert_eq!(links[0]["to_proposition_id"], "effects-2026");
    assert_eq!(links[0]["directed"], true);
    assert_eq!(links[0]["review_status"], "accepted");
    assert_eq!(
        report["relationship_history"][0]["relationships"][0]["review_status"],
        "suggested"
    );
    assert!(report.get("soft_truth").is_none());
}

#[test]
fn equivalence_and_overlap_are_undirected_and_keep_both_wordings() {
    let mut document = snapshot();
    document["relationships"][0]["kind"] = json!("partial_overlap");
    let report = run(&investigation(vec![("Overlap", document)])).unwrap();
    assert_eq!(report["relationships"][0]["directed"], false);
    assert_eq!(report["propositions"].as_array().unwrap().len(), 2);
    assert_eq!(
        proposition(&report, "effects-2026")["proposition"]["text"],
        "The intervention's effects last through 2026"
    );
}

#[test]
fn q_cannot_borrow_p_observations_for_an_attribution() {
    let mut document = snapshot();
    document["assessments"].as_array_mut().unwrap().push(json!({
        "propositionId": "effects-2026", "personId": "blogger-a", "stance": "endorses",
        "confidencePerson": {"value": 0.9, "assessedBy": "analyst"},
        "observationIds": ["a-stated"]
    }));
    assert!(run(&investigation(vec![("Borrow", document)]))
        .unwrap_err()
        .contains("unknown assessment observation"));
}

#[test]
fn relationship_claims_are_immutable_and_corrections_keep_history() {
    let mut changed = snapshot();
    changed["relationships"][0]["context"] = json!("A different population");
    assert!(run(&investigation(vec![
        ("First", snapshot()),
        ("Edit", changed)
    ]))
    .unwrap_err()
    .contains("new id"));

    let mut corrected = snapshot();
    corrected["relationships"][0] = implication();
    corrected["relationships"][0]["id"] = json!("overlap-corrected");
    corrected["relationships"][0]["kind"] = json!("partial_overlap");
    corrected["relationships"][0]["supersedes"] = json!("lasts-2027-implies-2026");
    let report = run(&investigation(vec![
        ("First", snapshot()),
        ("Correct the relationship type", corrected),
    ]))
    .unwrap();
    assert_eq!(
        report["relationships"][0]["supersedes"],
        "lasts-2027-implies-2026"
    );
    assert_eq!(
        report["relationship_history"][0]["relationships"][0]["id"],
        "lasts-2027-implies-2026"
    );

    let mut rejected = snapshot();
    rejected["relationships"][0]["review"] = json!({"status": "rejected"});
    let report = run(&investigation(vec![
        ("First", snapshot()),
        ("Reject", rejected),
    ]))
    .unwrap();
    assert_eq!(report["relationships"][0]["review_status"], "rejected");
}

#[test]
fn a_proposition_added_later_starts_its_own_history() {
    let mut first = snapshot();
    first["propositions"].as_array_mut().unwrap().pop();
    first["relationships"] = json!([]);
    let report = run(&investigation(vec![
        ("Start with P", first.clone()),
        ("Add Q and a suggested link", snapshot()),
    ]))
    .unwrap();
    let q = proposition(&report, "effects-2026");
    assert_eq!(q["since_revision"], 2);
    assert_eq!(q["history"], json!([]));
    assert_eq!(
        proposition(&report, "effects-2027")["history"][0]["revision"],
        1
    );
    assert_eq!(report["revision"], 2);
}

type Mutation = Box<dyn Fn(&mut Value)>;

#[test]
fn malformed_investigations_are_rejected() {
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "distinct propositions",
            Box::new(|d| d["relationships"][0]["toPropositionId"] = json!("effects-2027")),
        ),
        (
            "distinct propositions",
            Box::new(|d| d["relationships"][0]["toPropositionId"] = json!("missing")),
        ),
        (
            "unsupported relationship kind",
            Box::new(|d| d["relationships"][0]["kind"] = json!("contradiction")),
        ),
        (
            "suggestedBy",
            Box::new(|d| d["relationships"][0]["suggestedBy"] = json!("model")),
        ),
        (
            "review.status",
            Box::new(|d| d["relationships"][0]["review"]["status"] = json!("merged")),
        ),
        (
            "relationship.context",
            Box::new(|d| d["relationships"][0]["context"] = json!(" ")),
        ),
        (
            "needs a propositionId",
            Box::new(|d| {
                d["observations"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("propositionId");
            }),
        ),
        (
            "duplicate proposition",
            Box::new(|d| {
                let p = d["propositions"][0].clone();
                d["propositions"].as_array_mut().unwrap().push(p);
            }),
        ),
    ];
    for (expected, mutate) in cases {
        let mut document = snapshot();
        mutate(&mut document);
        let error = run(&investigation(vec![("Bad", document)])).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }

    let mut renamed = snapshot();
    renamed["propositions"][1]["text"] = json!("Something else");
    assert!(run(&investigation(vec![
        ("First", snapshot()),
        ("Rename", renamed)
    ]))
    .unwrap_err()
    .contains("silently change"));
    let mut removed = snapshot();
    removed["propositions"].as_array_mut().unwrap().pop();
    removed["relationships"] = json!([]);
    assert!(run(&investigation(vec![
        ("First", snapshot()),
        ("Remove", removed)
    ]))
    .unwrap_err()
    .contains("cannot be removed"));
}
