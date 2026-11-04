//! Offline stage-4 checks with a tiny two-debater fixture and a precomputed similarity table
//! (no embedding model). belief-lab#63.
use belief_cli::attribution::{compare, ComparisonInput};
use golden_scoring::interchange::{belief_lab_export, ExportProvenance};
use golden_scoring::stage4::{
    build_document, map_candidates, resolve_holder, score_attributions, summary_markdown,
    Candidate, ScoringConfig, Similarities, StageFourDocument,
};
use golden_scoring::{GoldenExpectations, PredictedTurn, TimedText};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const THRESHOLD: f64 = 0.85;

fn golden() -> GoldenExpectations {
    GoldenExpectations::parse(
        &json!({
            "status": "approved",
            "source": {"url": "u", "durationSeconds": 200},
            "people": [
                {"id": "narrator", "displayName": "Narrator (BBC announcer)"},
                {"id": "copleston", "displayName": "Frederick Copleston"},
                {"id": "russell", "displayName": "Bertrand Russell"}
            ],
            "speakerTurns": [
                {"speaker": "narrator", "startSeconds": 0, "endSeconds": 60, "evidence": "intro"},
                {"speaker": "copleston", "startSeconds": 60, "endSeconds": 120, "evidence": "c"},
                {"speaker": "russell", "startSeconds": 120, "endSeconds": 200, "evidence": "r"}
            ],
            "propositions": [
                {"id": "god-exists", "text": "God exists.", "scope": "s"},
                {"id": "world-explained", "text": "The world has an explanation.", "scope": "s"},
                {"id": "existence-pred", "text": "Existence is not a predicate.", "scope": "s"}
            ],
            "expectedAttributions": [
                {"person": "copleston", "proposition": "god-exists", "stance": "endorses",
                 "kind": "reported_position", "via": "narrator", "approxSeconds": 10, "quote": "holds that God exists"},
                {"person": "copleston", "proposition": "world-explained", "stance": "endorses",
                 "kind": "explicit_endorsement", "approxSeconds": 70, "quote": "the world has an explanation"},
                {"person": "russell", "proposition": "world-explained", "stance": "endorses_negation",
                 "kind": "explicit_endorsement", "approxSeconds": 130, "quote": "the world has no explanation"},
                {"person": "russell", "proposition": "existence-pred", "stance": "endorses",
                 "kind": "explicit_endorsement", "approxSeconds": 30, "quote": "not a predicate"},
                {"person": "copleston", "proposition": "existence-pred", "stance": "no_stance_evidence"}
            ],
            "expectedNoAttribution": [
                {"person": "russell", "proposition": "god-exists", "reason": "only reported"}
            ]
        })
        .to_string(),
    )
    .unwrap()
}

fn text(start_seconds: f64, end_seconds: f64, text: &str) -> TimedText {
    TimedText {
        start_seconds,
        end_seconds,
        text: text.into(),
    }
}

fn document() -> StageFourDocument {
    let transcript = [
        text(
            0.0,
            20.0,
            " Copleston is a believer who holds that God exists.",
        ),
        text(20.0, 40.0, " Nobody knows."),
        text(60.0, 80.0, " I think the world has an explanation."),
        text(80.0, 100.0, " Perhaps it is caused."),
        text(
            120.0,
            140.0,
            " The world has no explanation. God does not exist.",
        ),
    ];
    let turns = [
        PredictedTurn {
            speaker: "SPEAKER_00".into(),
            start_seconds: 0.0,
            end_seconds: 50.0,
        },
        PredictedTurn {
            speaker: "SPEAKER_01".into(),
            start_seconds: 55.0,
            end_seconds: 110.0,
        },
        PredictedTurn {
            speaker: "SPEAKER_02".into(),
            start_seconds: 115.0,
            end_seconds: 150.0,
        },
    ];
    let mapping = BTreeMap::from([
        ("SPEAKER_00".to_string(), "narrator".to_string()),
        ("SPEAKER_01".to_string(), "copleston".to_string()),
        ("SPEAKER_02".to_string(), "russell".to_string()),
    ]);
    build_document("case", "en", &transcript, &turns, &mapping)
}

/// A candidate quoting `quote` (first occurrence) with the given reporting chain.
fn candidate(
    document: &StageFourDocument,
    quote: &str,
    canonical: &str,
    stance: &str,
    reporting: &[(&str, &str)],
) -> Value {
    let start = document.text.find(quote).unwrap();
    let segment = document
        .segment_containing(start, start + quote.len())
        .unwrap();
    json!({
        "canonicalText": canonical,
        "evidence": {"quote": quote, "start": start, "end": start + quote.len(), "segmentId": segment.id},
        "speaker": segment.speaker,
        "reporting": reporting.iter().map(|(holder, cue)| {
            let at = document.text.find(cue).unwrap();
            json!({"holder": holder, "cue": cue, "cueSpan": {"start": at, "end": at + cue.len()}})
        }).collect::<Vec<_>>(),
        "stanceCue": stance,
        "needsRefinement": []
    })
}

fn candidates(document: &StageFourDocument) -> Vec<Candidate> {
    let values = vec![
        // 0: narrator reports Copleston's belief.
        candidate(
            document,
            "God exists",
            "God exists.",
            "asserted",
            &[("Copleston", "holds")],
        ),
        // 1: Copleston's own "I think" is his assertion.
        candidate(
            document,
            "the world has an explanation",
            "The world has an explanation.",
            "asserted",
            &[("I", "think")],
        ),
        // 2: hedged, never suspension.
        candidate(
            document,
            "Perhaps it is caused",
            "The world is caused.",
            "hedged",
            &[],
        ),
        // 3: Russell denies world-explained.
        candidate(
            document,
            "The world has no explanation",
            "The world has an explanation.",
            "negated",
            &[],
        ),
        // 4: Russell in his own words on god-exists: violates expectedNoAttribution.
        candidate(
            document,
            "God does not exist",
            "God exists.",
            "negated",
            &[],
        ),
    ];
    let mut parsed: Vec<Candidate> = values
        .into_iter()
        .map(|value| serde_json::from_value(value).unwrap())
        .collect();
    // 5: a quote that is not the transcript bytes is never attributed.
    let mut forged = parsed[3].clone();
    forged.evidence.as_mut().unwrap().quote = "The world has an explanation".into();
    parsed.push(forged);
    parsed
}

fn similarities() -> Similarities {
    let row = |pairs: &[(&str, f64)]| {
        pairs
            .iter()
            .map(|(id, value)| ((*id).to_string(), *value))
            .collect::<BTreeMap<_, _>>()
    };
    Similarities {
        model: "fixture".into(),
        revision: "fixture".into(),
        input_kind: "symmetric".into(),
        candidates: vec![
            row(&[
                ("god-exists", 0.99),
                ("world-explained", 0.7),
                ("existence-pred", 0.6),
            ]),
            row(&[
                ("god-exists", 0.7),
                ("world-explained", 0.98),
                ("existence-pred", 0.6),
            ]),
            row(&[
                ("god-exists", 0.7),
                ("world-explained", 0.82),
                ("existence-pred", 0.6),
            ]),
            row(&[
                ("god-exists", 0.7),
                ("world-explained", 0.95),
                ("existence-pred", 0.6),
            ]),
            row(&[
                ("god-exists", 0.97),
                ("world-explained", 0.7),
                ("existence-pred", 0.6),
            ]),
            row(&[
                ("god-exists", 0.7),
                ("world-explained", 0.95),
                ("existence-pred", 0.6),
            ]),
        ],
    }
}

#[test]
fn the_document_keeps_turns_bytes_and_times() {
    let document = document();
    assert_eq!(document.segments.len(), 3);
    assert_eq!(document.segments[0].speaker.as_deref(), Some("narrator"));
    assert!(!document.text.starts_with(' ') && !document.text.ends_with(' '));
    let second = document.text.find("Nobody").unwrap();
    assert!((document.seconds_at(second).unwrap() - 20.0).abs() < 1e-9);
    let segment = &document.segments[1];
    assert_eq!(
        &document.text[segment.start..segment.end],
        "I think the world has an explanation. Perhaps it is caused."
    );
    let input = document.extraction_input();
    assert_eq!(input["document"]["segments"][2]["speaker"], "russell");
}

#[test]
fn holders_resolve_only_to_one_golden_person() {
    let people = golden().people;
    assert_eq!(
        resolve_holder("Father Copleston", "narrator", &people).as_deref(),
        Some("copleston")
    );
    assert_eq!(
        resolve_holder("Russell's", "narrator", &people).as_deref(),
        Some("russell")
    );
    assert_eq!(
        resolve_holder("I", "russell", &people).as_deref(),
        Some("russell")
    );
    assert_eq!(resolve_holder("Sartre", "copleston", &people), None);
    assert_eq!(resolve_holder("he", "copleston", &people), None);
}

#[test]
fn candidates_map_with_the_owner_stance_map() {
    let document = document();
    let mapped = map_candidates(&document, &golden().people, &candidates(&document));
    assert_eq!(mapped.verified, 5);
    assert_eq!(mapped.unattributed.len(), 1);
    assert_eq!(mapped.unattributed[0].reason, "quote_not_transcript_bytes");

    let of = |candidate: usize| {
        mapped
            .attributions
            .iter()
            .filter(move |item| item.candidate == candidate)
            .collect::<Vec<_>>()
    };
    let reported = of(0);
    assert_eq!(reported.len(), 2);
    assert_eq!(reported[0].person, "narrator");
    assert_eq!(reported[0].stance, "no_stance");
    assert_eq!(reported[0].kind, "reported_position");
    assert_eq!(reported[0].about_person.as_deref(), Some("copleston"));
    assert_eq!(reported[0].attributed_stance.as_deref(), Some("endorses"));
    assert_eq!(reported[1].person, "copleston");
    assert_eq!(reported[1].stance, "endorses");
    assert_eq!(reported[1].via.as_deref(), Some("narrator"));

    let own = of(1);
    assert_eq!(own.len(), 1, "'I think' is the speaker's own assertion");
    assert_eq!(own[0].kind, "explicit_endorsement");

    let hedged = of(2);
    assert_eq!(hedged[0].stance, "no_stance");
    assert!(hedged[0].hedged);
    assert_eq!(of(3)[0].stance, "endorses_negation");
    assert!((of(3)[0].seconds - 120.0).abs() < 1e-9);
}

#[test]
fn scoring_reports_hits_misses_fabrications_and_the_no_attribution_rule() {
    let golden = golden();
    let document = document();
    let mapped = map_candidates(&document, &golden.people, &candidates(&document));
    let score = score_attributions(
        &golden,
        &mapped,
        &similarities(),
        ScoringConfig::with_threshold(THRESHOLD),
    );
    let status: Vec<&str> = score
        .expected
        .iter()
        .map(|result| result.status.as_str())
        .collect();
    assert_eq!(status, ["hit", "hit", "hit", "miss", "held"]);
    assert_eq!((score.hits, score.partial, score.misses), (3, 0, 1));

    assert!(!score.no_attribution[0].held);
    assert_eq!(score.no_attribution[0].violations[0].candidate, 4);
    assert_eq!(score.fabricated.len(), 1);
    assert_eq!(score.fabricated[0].reason, "contradicts_no_stance");
    assert_eq!(score.fabricated[0].predicted.person, "russell");

    // The hedged claim is 0.03 below the threshold near Copleston's world-explained entry.
    assert!(score.near_misses.iter().any(|miss| miss.candidate == 2
        && miss.reason == "similarity"
        && miss.expected_index == 1));
    let summary = summary_markdown(&score);
    assert!(summary.contains("3 hit, 0 partial, 1 miss of 4"));
    assert!(summary.contains("VIOLATED"));
}

#[test]
fn a_wrong_stance_is_partial_and_fabricated() {
    let golden = golden();
    let document = document();
    let mut candidates = candidates(&document);
    candidates[3].stance_cue = Some("asserted".into());
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let score = score_attributions(
        &golden,
        &mapped,
        &similarities(),
        ScoringConfig::with_threshold(THRESHOLD),
    );
    assert_eq!(score.expected[2].status, "partial");
    let best = score.expected[2].best.as_ref().unwrap();
    assert!(best.person_ok && !best.stance_ok);
    assert!(score
        .fabricated
        .iter()
        .any(|item| item.reason == "wrong_stance" && item.predicted.candidate == 3));
}

#[test]
fn the_export_is_accepted_by_belief_lab_attribution_compare() {
    let golden = golden();
    let document = document();
    let mapped = map_candidates(&document, &golden.people, &candidates(&document));
    let (batch, investigation) = belief_lab_export(
        &golden,
        &document,
        &mapped.attributions,
        &similarities(),
        THRESHOLD,
        &ExportProvenance {
            exporter_revision: "git:fixture".into(),
            source_repository: "belief-lab-golden".into(),
            scope_id: "case".into(),
            transcript_revision: "sha256:fixture".into(),
            producer_name: "proposition-api".into(),
            producer_revision: "git:fixture".into(),
            producer_model: "fixture".into(),
        },
    );
    // The holder's evidence derives from the reporter's: the `via` chain survives as parentage.
    let holder = batch["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["subject"] == "copleston" && item["familyId"] == "case:candidate-0")
        .unwrap();
    assert_eq!(holder["parentEvidenceIds"], json!(["e-0001"]));

    let report = compare(&ComparisonInput {
        evidence_json: &batch.to_string(),
        attributions_json: &investigation.to_string(),
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), "semantic_research".into())]),
    })
    .unwrap();
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(report["report"], "person_belief_investigation");
    let ids: Vec<&str> = report["propositions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["proposition"]["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"god-exists") && ids.contains(&"candidate-2"));
}
