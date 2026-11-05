//! Offline stage-4 checks with a tiny two-debater fixture and a precomputed similarity table
//! (no embedding model). belief-lab#63.
use belief_cli::attribution::{compare, ComparisonInput};
use golden_scoring::interchange::{belief_lab_export, ExportProvenance};
use golden_scoring::stage4::{
    build_document, map_candidates, resolve_holder, score_attributions, score_with_polarity,
    summary_markdown, Candidate, NliJudgment, NliLabel, Polarity, ScoringConfig, Similarities,
    StageFourDocument,
};
use golden_scoring::stage4_diagnostics::{diagnose_stage4, diagnostics_markdown};
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
fn stage4_diagnostics_trace_every_expected_and_fabricated_case_without_models() {
    let golden = golden();
    let document = document();
    let candidates = candidates(&document);
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let similarities = similarities();
    let score = score_attributions(
        &golden,
        &mapped,
        &similarities,
        ScoringConfig::with_threshold(THRESHOLD),
    );
    let diagnostic = diagnose_stage4(&document, &candidates, &mapped, &similarities, None, &score);
    assert_eq!(
        diagnostic.expected.len(),
        golden.expected_attributions.len()
    );
    assert_eq!(diagnostic.fabricated.len(), score.fabricated.len());
    assert_eq!(diagnostic.no_attribution.len(), 1);
    assert!(!diagnostic.no_attribution[0].held);
    assert_eq!(diagnostic.no_attribution[0].violations[0].candidate, 4);
    assert!(!diagnostic.expected[3].observed_blockers.is_empty());
    assert!(diagnostic.expected[3]
        .candidate_traces
        .iter()
        .any(|trace| trace.source_quote.is_some() && trace.similarity.is_some()));
    let fabrication = &diagnostic.fabricated[0];
    assert_eq!(fabrication.fabrication.reason, "contradicts_no_stance");
    assert_eq!(fabrication.candidate_trace.candidate, 4);
    assert!(fabrication.candidate_trace.source_span.is_some());
    assert_eq!(
        fabrication.candidate_trace.speaker.as_deref(),
        Some("russell")
    );
    let repeat = diagnose_stage4(&document, &candidates, &mapped, &similarities, None, &score);
    assert_eq!(
        serde_json::to_value(&diagnostic).unwrap(),
        serde_json::to_value(&repeat).unwrap()
    );
    let markdown = diagnostics_markdown(&diagnostic);
    assert!(markdown.contains("Expected #4"));
    assert!(markdown.contains("Fabricated attributions"));
}

#[test]
fn stage4_diagnostics_keep_nli_evidence_and_wrong_stance_evidence() {
    let golden = golden();
    let document = document();
    let mut candidates = candidates(&document);
    candidates[3].stance_cue = Some("asserted".into());
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let similarities = similarities();
    let polarity = polarity(&[(1, "world-explained", NliLabel::Neutral)]);
    let score = score_with_polarity(
        &golden,
        &mapped,
        &similarities,
        Some(&polarity),
        ScoringConfig::with_threshold(THRESHOLD),
    )
    .unwrap();
    let diagnostic = diagnose_stage4(
        &document,
        &candidates,
        &mapped,
        &similarities,
        Some(&polarity),
        &score,
    );
    // Flip-only polarity: a neutral judgment is evidence, not a blocker.
    assert_eq!(diagnostic.expected[1].status, "hit");
    let trace = diagnostic.expected[1]
        .candidate_traces
        .iter()
        .find(|trace| trace.candidate == 1)
        .unwrap();
    assert_eq!(trace.nli.as_ref().unwrap().label, NliLabel::Neutral);
    assert_eq!(trace.nli.as_ref().unwrap().probabilities, [0.1, 0.8, 0.1]);
    assert!(trace.failed_gates.is_empty(), "{:?}", trace.failed_gates);
    assert_eq!(
        diagnostic.expected[2].observed_blockers,
        vec!["stance".to_owned()]
    );
}

#[test]
fn stage4_diagnostics_always_include_forbidden_stance_violations() {
    let mut golden = golden();
    let no_stance = golden.expected_attributions.last_mut().unwrap();
    no_stance.person = "russell".into();
    no_stance.proposition = "god-exists".into();
    let document = document();
    let candidates = candidates(&document);
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let similarities = similarities();
    let score = score_attributions(
        &golden,
        &mapped,
        &similarities,
        ScoringConfig::with_threshold(THRESHOLD),
    );
    let diagnostic = diagnose_stage4(&document, &candidates, &mapped, &similarities, None, &score);
    let rule = diagnostic.expected.last().unwrap();
    assert_eq!(rule.status, "violated");
    assert_eq!(rule.observed_blockers, ["unexpected_stance_evidence"]);
    assert!(rule
        .candidate_traces
        .iter()
        .any(|trace| trace.candidate == 4));
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

#[test]
fn empty_evidence_is_never_attributed() {
    let document = document();
    let mut candidates = candidates(&document);
    let evidence = candidates[3].evidence.as_mut().unwrap();
    evidence.end = evidence.start;
    evidence.quote = String::new();
    let mapped = map_candidates(&document, &golden().people, &candidates);
    assert!(mapped
        .unattributed
        .iter()
        .any(|item| item.candidate == 3 && item.reason == "empty_evidence"));
    assert!(mapped.attributions.iter().all(|item| item.candidate != 3));
}

#[test]
fn a_nested_report_keeps_the_whole_reporter_chain() {
    let golden = golden();
    let document = document();
    // Narrator: "Copleston is a believer who holds that God exists", read as
    // narrator → Copleston → Russell for the chain.
    let nested: Candidate = serde_json::from_value(candidate(
        &document,
        "God exists",
        "God exists.",
        "asserted",
        &[("Copleston", "believer"), ("Russell", "holds")],
    ))
    .unwrap();
    let mapped = map_candidates(&document, &golden.people, &[nested]);
    let chain: Vec<(&str, &str, Option<&str>)> = mapped
        .attributions
        .iter()
        .map(|item| {
            (
                item.person.as_str(),
                item.stance.as_str(),
                item.via.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        chain,
        [
            ("narrator", "no_stance", None),
            ("copleston", "no_stance", Some("narrator")),
            ("russell", "endorses", Some("copleston")),
        ]
    );
    let (batch, _) = belief_lab_export(
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
    let parents: Vec<Value> = batch["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["parentEvidenceIds"].clone())
        .collect();
    assert_eq!(parents, [json!([]), json!(["e-0001"]), json!(["e-0002"])]);
}

#[test]
fn an_own_words_prohibition_is_fabricated_even_when_a_reported_entry_has_that_stance() {
    let mut golden = golden();
    // Copleston's endorsement of god-exists is approved only as the narrator's report.
    golden
        .expected_no_attribution
        .push(golden_scoring::GoldenNoAttribution {
            person: "copleston".into(),
            proposition: "god-exists".into(),
            reason: "only reported".into(),
        });
    let document = document();
    let mapped = map_candidates(&document, &golden.people, &candidates(&document));
    let mut similarities = similarities();
    // Copleston's own "I think …" claim (candidate 1) now matches god-exists.
    similarities.candidates[1].insert("god-exists".into(), 0.99);
    let score = score_attributions(
        &golden,
        &mapped,
        &similarities,
        ScoringConfig::with_threshold(THRESHOLD),
    );
    assert!(score
        .fabricated
        .iter()
        .any(|item| item.predicted.candidate == 1
            && item.proposition == "god-exists"
            && item.reason == "contradicts_no_stance"));
    assert!(!score.no_attribution[1].held);
}

fn judgment(label: NliLabel) -> NliJudgment {
    let probabilities = match label {
        NliLabel::Entailment => [0.9, 0.05, 0.05],
        NliLabel::Neutral => [0.1, 0.8, 0.1],
        NliLabel::Contradiction => [0.05, 0.05, 0.9],
    };
    NliJudgment {
        label,
        probabilities,
    }
}

/// A recorded NLI table for every pair at or above the threshold (fixture, no model).
fn polarity(overrides: &[(usize, &str, NliLabel)]) -> Polarity {
    let similarities = similarities();
    let mut candidates: Vec<BTreeMap<String, NliJudgment>> = similarities
        .candidates
        .iter()
        .map(|row| {
            row.iter()
                .filter(|(_, value)| **value >= THRESHOLD)
                .map(|(id, _)| (id.clone(), judgment(NliLabel::Entailment)))
                .collect()
        })
        .collect();
    for (candidate, proposition, label) in overrides {
        candidates[*candidate].insert((*proposition).to_string(), judgment(*label));
    }
    Polarity {
        model: "fixture-nli".into(),
        revision: "fixture".into(),
        candidates,
    }
}

#[test]
fn polarity_only_flips_confident_contradictions_and_neutral_keeps_the_match() {
    let golden = golden();
    let document = document();
    let mut candidates = candidates(&document);
    // Russell "asserts" a claim that contradicts world-explained: with NLI it is his negation.
    candidates[3].stance_cue = Some("asserted".into());
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let config = ScoringConfig::with_threshold(THRESHOLD);

    let baseline = score_attributions(&golden, &mapped, &similarities(), config);
    assert_eq!(baseline.expected[2].status, "partial");
    assert_eq!(baseline.polarity_model, None);

    let table = polarity(&[
        (3, "world-explained", NliLabel::Contradiction),
        (1, "world-explained", NliLabel::Neutral),
    ]);
    let flip_at = |min: f64| {
        score_with_polarity(
            &golden,
            &mapped,
            &similarities(),
            Some(&table),
            config.with_min_contradiction(min),
        )
        .unwrap()
    };
    let score = flip_at(0.8);
    assert_eq!(score.polarity_model.as_deref(), Some("fixture-nli@fixture"));
    assert_eq!(
        score.expected[2].status, "hit",
        "0.9 contradiction flips to endorses_negation"
    );
    assert_eq!(
        score.expected[1].status, "hit",
        "a neutral judgment keeps the cosine match"
    );
    // Below the confidence the stance is not flipped.
    assert_eq!(flip_at(0.95).expected[2].status, "partial");
}

#[test]
fn a_missing_nli_judgment_fails_instead_of_defaulting() {
    let golden = golden();
    let document = document();
    let mapped = map_candidates(&document, &golden.people, &candidates(&document));
    let mut table = polarity(&[]);
    table.candidates[0].remove("god-exists");
    let error = score_with_polarity(
        &golden,
        &mapped,
        &similarities(),
        Some(&table),
        ScoringConfig::with_threshold(THRESHOLD),
    )
    .unwrap_err();
    assert!(error.contains("candidate 0") && error.contains("god-exists"));
}

#[test]
fn the_default_config_never_flips_even_a_certain_contradiction() {
    let golden = golden();
    let document = document();
    let mut candidates = candidates(&document);
    candidates[3].stance_cue = Some("asserted".into());
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let mut table = polarity(&[(3, "world-explained", NliLabel::Contradiction)]);
    table.candidates[3]
        .get_mut("world-explained")
        .unwrap()
        .probabilities = [0.0, 0.0, 1.0];
    let score = |config: ScoringConfig| {
        score_with_polarity(&golden, &mapped, &similarities(), Some(&table), config).unwrap()
    };
    let default = ScoringConfig::with_threshold(THRESHOLD);
    assert_eq!(score(default).expected[2].status, "partial");
    // A report written before `minContradiction` existed replays as "never flip".
    let mut old = serde_json::to_value(default).unwrap();
    old.as_object_mut().unwrap().remove("minContradiction");
    let replayed: ScoringConfig = serde_json::from_value(old).unwrap();
    assert_eq!(score(replayed).expected[2].status, "partial");
    assert_eq!(
        score(default.with_min_contradiction(1.0)).expected[2].status,
        "hit"
    );
}
