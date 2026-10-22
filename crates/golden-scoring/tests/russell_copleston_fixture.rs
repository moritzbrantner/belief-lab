//! Offline checks of the committed Russell–Copleston golden fixture (belief-lab#63).
use golden_scoring::{score_turns, GoldenExpectations, PredictedTurn};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

fn case_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden/russell-copleston-1948")
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn committed_audio_matches_its_manifest_and_the_golden_diarization_input() {
    let manifest = read_json(case_dir().join("audio/source.json"));
    let bytes = std::fs::read(
        case_dir()
            .join("audio")
            .join(manifest["file"].as_str().unwrap()),
    )
    .unwrap();
    assert_eq!(bytes.len() as u64, manifest["bytes"].as_u64().unwrap());
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(digest, manifest["sha256"]);

    let golden = read_json(case_dir().join("expected-beliefs.json"));
    assert_eq!(golden["source"]["url"], manifest["source"]["url"]);
    assert_eq!(
        golden["speakerTurnEvidence"]["diarization"]["audioSha256"],
        manifest["decoded16kMonoWavSha256"],
        "the golden turns were checked against a decode of exactly these bytes"
    );
}

#[test]
fn golden_turns_are_approved_contiguous_and_score_perfectly_against_themselves() {
    let golden = GoldenExpectations::parse(
        &std::fs::read_to_string(case_dir().join("expected-beliefs.json")).unwrap(),
    )
    .unwrap();
    assert!(golden.is_approved());
    for pair in golden.speaker_turns.windows(2) {
        assert!(
            (pair[0].end_seconds - pair[1].start_seconds).abs() < 1e-9,
            "turns must be contiguous at {}",
            pair[0].end_seconds
        );
    }
    let relabelled: Vec<PredictedTurn> = golden
        .speaker_turns
        .iter()
        .map(|turn| PredictedTurn {
            speaker: format!("SPEAKER_{}", turn.speaker.len()),
            start_seconds: turn.start_seconds,
            end_seconds: turn.end_seconds,
        })
        .collect();
    let score = score_turns(&golden, &relabelled);
    assert!(score.agreement > 0.999_999, "{score:?}");
    assert!(score.speech_coverage > 0.999_999, "{score:?}");
    assert_eq!(score.label_mapping.len(), 3);
}
