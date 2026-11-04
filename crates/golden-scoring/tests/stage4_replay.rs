//! The CLI replays recorded tables without a model and refuses source or scoring drift.
use golden_scoring::stage4::{
    map_candidates, score_with_polarity, Candidate, DocumentSegment, NliJudgment, NliLabel,
    Polarity, ScoringConfig, Similarities, StageFourDocument, TimedSpan,
};
use golden_scoring::GoldenExpectations;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_RUN: AtomicU64 = AtomicU64::new(0);

fn write_json(dir: &Path, file: &str, value: &Value) {
    std::fs::write(dir.join(file), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn replay_produces_diagnostics_and_refuses_baseline_or_document_drift() {
    let dir = std::env::temp_dir().join(format!(
        "belief-lab-stage4-replay-{}-{}",
        std::process::id(),
        NEXT_RUN.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let golden_value = json!({
        "status": "approved",
        "source": {"url": "fixture", "durationSeconds": 10},
        "speakerTurns": [{"speaker": "russell", "startSeconds": 0, "endSeconds": 10, "evidence": "God exists"}],
        "people": [{"id": "russell", "displayName": "Bertrand Russell"}],
        "propositions": [{"id": "god-exists", "text": "God exists", "scope": "test"}],
        "expectedAttributions": [{"person": "russell", "proposition": "god-exists", "stance": "endorses",
                                  "kind": "explicit_endorsement", "approxSeconds": 0, "quote": "God exists"}]
    });
    let golden_path = dir.join("expected-beliefs.json");
    write_json(&dir, "expected-beliefs.json", &golden_value);
    let golden = GoldenExpectations::parse(&golden_value.to_string()).unwrap();
    let document = StageFourDocument {
        id: "test-document".into(),
        language: "en".into(),
        text: "God exists".into(),
        segments: vec![DocumentSegment {
            id: "turn-001".into(),
            speaker: Some("russell".into()),
            start: 0,
            end: 10,
        }],
        timing: vec![TimedSpan {
            start: 0,
            end: 10,
            start_seconds: 0.0,
            end_seconds: 10.0,
        }],
    };
    let doc_bytes = serde_json::to_vec_pretty(&document).unwrap();
    std::fs::write(dir.join("stage4-document.json"), &doc_bytes).unwrap();
    let doc_sha: String = Sha256::digest(&doc_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let candidates: Vec<Candidate> = serde_json::from_value(json!([{
        "canonicalText": "God exists",
        "evidence": {"quote": "God exists", "start": 0, "end": 10, "segmentId": "turn-001"},
        "speaker": "russell", "stanceCue": "asserted"
    }]))
    .unwrap();
    write_json(
        &dir,
        "stage4-candidates.json",
        &json!({"documentSha256": doc_sha, "candidates": candidates}),
    );
    let similarities = Similarities {
        model: "fixture-e5".into(),
        revision: "fixture".into(),
        input_kind: "symmetric".into(),
        candidates: vec![BTreeMap::from([("god-exists".into(), 0.99)])],
    };
    let polarity = Polarity {
        model: "fixture-nli".into(),
        revision: "fixture".into(),
        candidates: vec![BTreeMap::from([(
            "god-exists".into(),
            NliJudgment {
                label: NliLabel::Entailment,
                probabilities: [0.9, 0.05, 0.05],
            },
        )])],
    };
    write_json(
        &dir,
        "similarities.json",
        &serde_json::to_value(&similarities).unwrap(),
    );
    write_json(
        &dir,
        "polarity.json",
        &serde_json::to_value(&polarity).unwrap(),
    );
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let score = score_with_polarity(
        &golden,
        &mapped,
        &similarities,
        Some(&polarity),
        ScoringConfig::with_threshold(0.85),
    )
    .unwrap();
    write_json(&dir, "attributions-report.json", &json!({"score": score}));

    let command = || {
        Command::new(env!("CARGO_BIN_EXE_stage4-diagnostics"))
            .arg(&golden_path)
            .arg(&dir)
            .output()
            .unwrap()
    };
    let output = command();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(dir.join("attributions-diagnostics.json")).unwrap())
            .unwrap();
    assert_eq!(report["expected"].as_array().unwrap().len(), 1);
    assert_eq!(report["expected"][0]["status"], "hit");
    assert_eq!(
        report["expected"][0]["candidateTraces"][0]["nli"]["label"],
        "entailment"
    );

    let mut stale: Value =
        serde_json::from_slice(&std::fs::read(dir.join("attributions-report.json")).unwrap())
            .unwrap();
    stale["score"]["hits"] = json!(0);
    write_json(&dir, "attributions-report.json", &stale);
    let output = command();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("score differs"));

    write_json(&dir, "attributions-report.json", &json!({"score": score}));
    let mut changed = doc_bytes;
    changed.push(b' ');
    std::fs::write(dir.join("stage4-document.json"), changed).unwrap();
    let output = command();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("different document bytes"));
    std::fs::remove_dir_all(&dir).unwrap();
}
