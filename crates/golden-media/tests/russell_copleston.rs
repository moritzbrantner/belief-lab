//! Opt-in real-model golden test for the Russell–Copleston debate (belief-lab#63, stages 1–3).
//!
//! Stage 1 (acquire): the committed original yt-dlp audio stream, checked against its manifest.
//! Stage 2 (transcribe): Whisper medium through audio-analysis's native Candle provider.
//! Stage 3 (who spoke when): pyannote community-1 through audio-analysis's native runtime.
//! Both are scored against the owner-approved golden file; a per-stage report is written to
//! `target/golden/russell-copleston-1948/report.json`.
//!
//! Requires `GOLDEN_PYANNOTE_BUNDLE` (the approved local ONNX snapshot). Whisper medium is
//! downloaded from Hugging Face on first use. `GOLDEN_THREADS` caps CPU threads (default 4).
use audio_analysis_transcription::{transcribe, TranscriptionPipelineRequest};
use golden_scoring::{score_transcript, score_turns, GoldenExpectations, PredictedTurn, TimedText};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_TURN_AGREEMENT: f64 = 0.95;
const MIN_SPEAKER_AGREEMENT: f64 = 0.90;
const MIN_SPEECH_COVERAGE: f64 = 0.80;
const EXCERPT_WINDOW_SECONDS: f64 = 3.0;
const MIN_EXCERPT_WORD_RECALL: f64 = 0.8;
const MIN_EXCERPTS_FOUND: f64 = 0.90;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "runs Whisper medium and pyannote community-1 on 17 minutes of audio (CPU, tens of minutes)"]
fn russell_copleston_transcript_and_speaker_turns_match_the_golden_file() {
    let threads = std::env::var("GOLDEN_THREADS").unwrap_or_else(|_| "4".into());
    std::env::set_var("RAYON_NUM_THREADS", &threads);

    let case = root().join("fixtures/golden/russell-copleston-1948");
    let golden_json = std::fs::read_to_string(case.join("expected-beliefs.json")).unwrap();
    let golden = GoldenExpectations::parse(&golden_json).unwrap();
    assert!(
        golden.is_approved(),
        "score only against an approved golden file"
    );
    let golden_raw: Value = serde_json::from_str(&golden_json).unwrap();

    // Stage 1: acquire.
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(case.join("audio/source.json")).unwrap()).unwrap();
    let audio = case.join("audio").join(manifest["file"].as_str().unwrap());
    let audio_sha256 = sha256_hex(&std::fs::read(&audio).unwrap());
    assert_eq!(audio_sha256, manifest["sha256"], "committed audio changed");

    // Stages 2 and 3: one producer pipeline call (ASR, then diarization over the same audio).
    let bundle = PathBuf::from(std::env::var_os("GOLDEN_PYANNOTE_BUNDLE").expect(
        "GOLDEN_PYANNOTE_BUNDLE must name the approved pyannote community-1 ONNX snapshot",
    ));
    let bundle_report = audio_analysis_speakers::inspect_pyannote_community_diarization_bundle(
        audio_analysis_speakers::PyannoteCommunityDiarizationConfig {
            bundle_path: bundle.clone(),
            manifest_file: None,
            segmentation_model_file: None,
            embedding_model_file: None,
            plda_transform_file: None,
            plda_model_file: None,
            clustering_config_file: None,
            min_speakers: None,
            max_speakers: None,
            return_speaker_embeddings: false,
        },
    )
    .expect("pyannote bundle must pass the producer's inspection");
    assert_eq!(
        bundle_report.artifact_set_sha256,
        golden_raw["speakerTurnEvidence"]["diarization"]["nativeRuntime"]["artifactSetSha256"],
        "use the approved bundle the golden turns were checked against"
    );

    let request: TranscriptionPipelineRequest = serde_json::from_value(json!({
        "source": {"path": audio},
        "provider": {"kind": "candleWhisper", "modelId": "medium", "language": "en", "device": "cpu"},
        "diarization": {
            "enabled": true,
            "modelId": "pyannote/speaker-diarization-community-1",
            "pyannoteModelBundle": bundle,
        },
    }))
    .unwrap();
    let started = Instant::now();
    let response = transcribe(request).expect("audio-analysis transcription pipeline");
    let seconds = started.elapsed().as_secs_f64();

    let transcript: Vec<TimedText> = response
        .transcript
        .segments
        .iter()
        .filter_map(|segment| {
            Some(TimedText {
                start_seconds: segment.start_seconds()?,
                end_seconds: segment.end_seconds()?,
                text: segment.text.clone(),
            })
        })
        .collect();
    let diarization = response
        .diarization
        .as_ref()
        .expect("diarization was requested");
    let turns: Vec<PredictedTurn> = diarization
        .segments
        .iter()
        .map(|segment| PredictedTurn {
            speaker: segment.speaker.clone(),
            start_seconds: f64::from(segment.start_seconds),
            end_seconds: f64::from(segment.end_seconds),
        })
        .collect();

    let transcript_score = score_transcript(
        &golden,
        &transcript,
        EXCERPT_WINDOW_SECONDS,
        MIN_EXCERPT_WORD_RECALL,
    );
    let turn_score = score_turns(&golden, &turns);

    let out = root().join("target/golden/russell-copleston-1948");
    std::fs::create_dir_all(&out).unwrap();
    let report = json!({
        "case": "russell-copleston-1948",
        "threads": threads,
        "pipelineSeconds": seconds,
        "stages": {
            "acquire": {"audioSha256": audio_sha256, "matchesManifest": true, "ytDlpFormatId": manifest["stream"]["ytDlpFormatId"]},
            "transcribe": {
                "provider": response.provider,
                "modelId": response.model_id,
                "segments": transcript.len(),
                "score": transcript_score,
            },
            "speakerTurns": {
                "modelId": diarization.model_id,
                "artifactSetSha256": bundle_report.artifact_set_sha256,
                "producerSegments": turns.len(),
                "score": turn_score,
            },
            "attributions": {"status": "not_run", "reason": "belief-lab#63 PR 2"},
        },
        "diagnostics": response.diagnostics,
    });
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    std::fs::write(
        out.join("pipeline-response.json"),
        serde_json::to_string_pretty(&response).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    assert!(
        transcript_score.found_share >= MIN_EXCERPTS_FOUND,
        "golden excerpts found in the transcript: {:.3}",
        transcript_score.found_share
    );
    assert!(
        turn_score.speech_coverage >= MIN_SPEECH_COVERAGE,
        "speech coverage {:.3}",
        turn_score.speech_coverage
    );
    assert!(
        turn_score.agreement >= MIN_TURN_AGREEMENT,
        "turn agreement {:.3}",
        turn_score.agreement
    );
    for (speaker, agreement) in &turn_score.per_speaker {
        assert!(
            agreement.agreement >= MIN_SPEAKER_AGREEMENT,
            "{speaker}: agreement {:.3}",
            agreement.agreement
        );
    }
}
