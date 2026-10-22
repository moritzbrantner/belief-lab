//! Scores real-model output against an owner-approved golden expectations file
//! (`fixtures/golden/<case>/expected-beliefs.json`).
//!
//! Scoring is pure and deterministic, so it runs offline in `cargo test --workspace`.
//! The model runs that feed it are opt-in (`crates/golden-media`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Frame step used for every time-based comparison.
pub const FRAME_SECONDS: f64 = 0.01;

/// The parts of a golden expectations file this crate scores.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenExpectations {
    pub status: String,
    pub source: GoldenSource,
    pub speaker_turns: Vec<GoldenTurn>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenSource {
    pub url: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenTurn {
    pub speaker: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub evidence: String,
}

impl GoldenExpectations {
    pub fn parse(json: &str) -> Result<Self, String> {
        let golden: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if golden.speaker_turns.is_empty() {
            return Err("golden file has no speaker turns".into());
        }
        for turn in &golden.speaker_turns {
            if turn.end_seconds <= turn.start_seconds {
                return Err(format!(
                    "golden turn {}..{} is empty",
                    turn.start_seconds, turn.end_seconds
                ));
            }
        }
        Ok(golden)
    }

    pub fn is_approved(&self) -> bool {
        self.status == "approved"
    }
}

/// One producer-labelled speaker segment (e.g. pyannote `SPEAKER_00`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictedTurn {
    pub speaker: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// One timed transcript segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedText {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAgreement {
    /// Seconds in which the golden file names this speaker and diarization detects speech.
    pub detected_seconds: f64,
    /// Share of `detected_seconds` where a mapped producer label equals this speaker.
    pub agreement: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnScore {
    /// Producer label → golden speaker, chosen to maximize agreement (one-to-one).
    /// Labels mapped to no golden speaker are listed in `unmappedLabels`.
    pub label_mapping: BTreeMap<String, String>,
    pub unmapped_labels: Vec<String>,
    /// Share of golden speech time in which diarization detects any speech.
    pub speech_coverage: f64,
    /// Share of detected golden speech time with the right (mapped) speaker.
    pub agreement: f64,
    pub per_speaker: BTreeMap<String, SpeakerAgreement>,
}

/// Scores diarized turns against the golden speaker turns.
///
/// Golden turns are contiguous and include pauses, so only frames in which diarization
/// detects speech are compared; `speechCoverage` reports how much golden time that is.
/// A frame agrees when any active producer label maps to the golden speaker. The mapping
/// from producer labels to golden speakers is the one-to-one assignment with the most
/// agreeing frames, so label names and order carry no meaning.
pub fn score_turns(golden: &GoldenExpectations, predicted: &[PredictedTurn]) -> TurnScore {
    let mut speakers: Vec<&str> = golden
        .speaker_turns
        .iter()
        .map(|turn| turn.speaker.as_str())
        .collect();
    speakers.sort_unstable();
    speakers.dedup();
    let mut labels: Vec<&str> = predicted.iter().map(|turn| turn.speaker.as_str()).collect();
    labels.sort_unstable();
    labels.dedup();

    let frames = (golden.source.duration_seconds / FRAME_SECONDS).ceil() as usize;
    // overlap[label][speaker] = frames where the label is active and golden names the speaker.
    let mut overlap = vec![vec![0usize; speakers.len()]; labels.len()];
    let mut golden_frames = vec![0usize; speakers.len()];
    let mut detected = vec![0usize; speakers.len()];
    let mut frame_labels: Vec<Vec<usize>> = Vec::with_capacity(frames);
    let mut frame_speaker: Vec<Option<usize>> = Vec::with_capacity(frames);
    for frame in 0..frames {
        let time = (frame as f64 + 0.5) * FRAME_SECONDS;
        let speaker = golden
            .speaker_turns
            .iter()
            .find(|turn| time >= turn.start_seconds && time < turn.end_seconds)
            .map(|turn| speakers.binary_search(&turn.speaker.as_str()).unwrap());
        let mut active: Vec<usize> = predicted
            .iter()
            .filter(|turn| time >= turn.start_seconds && time < turn.end_seconds)
            .map(|turn| labels.binary_search(&turn.speaker.as_str()).unwrap())
            .collect();
        active.sort_unstable();
        active.dedup();
        if let Some(speaker) = speaker {
            golden_frames[speaker] += 1;
            if !active.is_empty() {
                detected[speaker] += 1;
            }
            for &label in &active {
                overlap[label][speaker] += 1;
            }
        }
        frame_labels.push(active);
        frame_speaker.push(speaker);
    }

    let assignment = best_assignment(&overlap, speakers.len());
    let mut agreeing = vec![0usize; speakers.len()];
    for (active, speaker) in frame_labels.iter().zip(&frame_speaker) {
        if let Some(speaker) = *speaker {
            if active
                .iter()
                .any(|&label| assignment[label] == Some(speaker))
            {
                agreeing[speaker] += 1;
            }
        }
    }

    let ratio = |part: usize, whole: usize| {
        if whole == 0 {
            0.0
        } else {
            part as f64 / whole as f64
        }
    };
    let mut label_mapping = BTreeMap::new();
    let mut unmapped_labels = Vec::new();
    for (label, mapped) in labels.iter().zip(&assignment) {
        match mapped {
            Some(speaker) => {
                label_mapping.insert((*label).to_string(), speakers[*speaker].to_string());
            }
            None => unmapped_labels.push((*label).to_string()),
        }
    }
    let per_speaker = speakers
        .iter()
        .enumerate()
        .map(|(index, speaker)| {
            (
                (*speaker).to_string(),
                SpeakerAgreement {
                    detected_seconds: detected[index] as f64 * FRAME_SECONDS,
                    agreement: ratio(agreeing[index], detected[index]),
                },
            )
        })
        .collect();
    TurnScore {
        label_mapping,
        unmapped_labels,
        speech_coverage: ratio(detected.iter().sum(), golden_frames.iter().sum()),
        agreement: ratio(agreeing.iter().sum(), detected.iter().sum()),
        per_speaker,
    }
}

/// Exhaustive one-to-one assignment of labels to speakers (or to none) maximizing overlap.
/// Label and speaker counts are tiny (a handful), so exhaustive search is exact and cheap.
fn best_assignment(overlap: &[Vec<usize>], speakers: usize) -> Vec<Option<usize>> {
    fn search(
        label: usize,
        overlap: &[Vec<usize>],
        used: &mut Vec<bool>,
        current: &mut Vec<Option<usize>>,
        best: &mut (usize, Vec<Option<usize>>),
        total: usize,
    ) {
        if label == overlap.len() {
            if total > best.0 || best.1.is_empty() {
                *best = (total, current.clone());
            }
            return;
        }
        current.push(None);
        search(label + 1, overlap, used, current, best, total);
        current.pop();
        for speaker in 0..used.len() {
            if !used[speaker] {
                used[speaker] = true;
                current.push(Some(speaker));
                search(
                    label + 1,
                    overlap,
                    used,
                    current,
                    best,
                    total + overlap[label][speaker],
                );
                current.pop();
                used[speaker] = false;
            }
        }
    }
    if overlap.is_empty() {
        return Vec::new();
    }
    let mut best = (0, Vec::new());
    search(
        0,
        overlap,
        &mut vec![false; speakers],
        &mut Vec::new(),
        &mut best,
        0,
    );
    best.1
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcerptMatch {
    pub speaker: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub excerpt: String,
    /// Share of the excerpt's words found in the transcript near the turn.
    pub word_recall: f64,
    pub found: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptScore {
    pub window_seconds: f64,
    pub min_word_recall: f64,
    pub excerpts: Vec<ExcerptMatch>,
    /// Share of quoted golden excerpts found in the transcript.
    pub found_share: f64,
}

/// Checks that each quoted excerpt in the golden turns' evidence appears in the transcript
/// within `window_seconds` of its turn. Unquoted evidence (descriptions) is not scored.
/// An excerpt split by `...` is scored per fragment.
pub fn score_transcript(
    golden: &GoldenExpectations,
    transcript: &[TimedText],
    window_seconds: f64,
    min_word_recall: f64,
) -> TranscriptScore {
    let mut excerpts = Vec::new();
    for turn in &golden.speaker_turns {
        let window: Vec<String> = transcript
            .iter()
            .filter(|segment| {
                segment.end_seconds >= turn.start_seconds - window_seconds
                    && segment.start_seconds <= turn.end_seconds + window_seconds
            })
            .flat_map(|segment| words(&segment.text))
            .collect();
        for fragment in quoted_fragments(&turn.evidence) {
            let wanted = words(&fragment);
            if wanted.is_empty() {
                continue;
            }
            let hits = wanted.iter().filter(|word| window.contains(word)).count();
            let word_recall = hits as f64 / wanted.len() as f64;
            excerpts.push(ExcerptMatch {
                speaker: turn.speaker.clone(),
                start_seconds: turn.start_seconds,
                end_seconds: turn.end_seconds,
                excerpt: fragment,
                word_recall,
                found: word_recall >= min_word_recall,
            });
        }
    }
    let found = excerpts.iter().filter(|excerpt| excerpt.found).count();
    TranscriptScore {
        window_seconds,
        min_word_recall,
        found_share: if excerpts.is_empty() {
            0.0
        } else {
            found as f64 / excerpts.len() as f64
        },
        excerpts,
    }
}

fn quoted_fragments(evidence: &str) -> Vec<String> {
    evidence
        .split('"')
        .skip(1)
        .step_by(2)
        .flat_map(|quote| quote.split("..."))
        .map(|fragment| fragment.trim().to_string())
        .filter(|fragment| !fragment.is_empty())
        .collect()
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('’', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .map(|word| word.trim_matches('\''))
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden() -> GoldenExpectations {
        GoldenExpectations::parse(
            r#"{"status":"approved","source":{"url":"u","durationSeconds":10},
            "speakerTurns":[
              {"speaker":"a","startSeconds":0,"endSeconds":4,"evidence":"\"hello there ... general\""},
              {"speaker":"b","startSeconds":4,"endSeconds":10,"evidence":"describes something"}]}"#,
        )
        .unwrap()
    }

    fn turn(speaker: &str, start_seconds: f64, end_seconds: f64) -> PredictedTurn {
        PredictedTurn {
            speaker: speaker.into(),
            start_seconds,
            end_seconds,
        }
    }

    #[test]
    fn label_names_carry_no_meaning() {
        let score = score_turns(
            &golden(),
            &[turn("SPEAKER_01", 0.0, 4.0), turn("SPEAKER_00", 4.0, 10.0)],
        );
        assert_eq!(score.label_mapping["SPEAKER_01"], "a");
        assert_eq!(score.label_mapping["SPEAKER_00"], "b");
        assert!((score.agreement - 1.0).abs() < 1e-9);
        assert!((score.speech_coverage - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pauses_are_not_scored_but_reduce_coverage() {
        let score = score_turns(&golden(), &[turn("x", 0.0, 2.0), turn("y", 5.0, 10.0)]);
        assert!((score.agreement - 1.0).abs() < 1e-9);
        assert!((score.speech_coverage - 0.7).abs() < 1e-6);
    }

    #[test]
    fn wrong_speaker_lowers_that_speakers_agreement() {
        // Label x covers a fully and the first second of b.
        let score = score_turns(&golden(), &[turn("x", 0.0, 5.0), turn("y", 5.0, 10.0)]);
        assert!((score.per_speaker["a"].agreement - 1.0).abs() < 1e-9);
        assert!((score.per_speaker["b"].agreement - 5.0 / 6.0).abs() < 1e-6);
        assert!((score.agreement - 0.9).abs() < 1e-6);
    }

    #[test]
    fn extra_labels_stay_unmapped_and_never_agree() {
        let score = score_turns(
            &golden(),
            &[
                turn("x", 0.0, 4.0),
                turn("y", 4.0, 9.0),
                turn("z", 9.0, 10.0),
            ],
        );
        assert_eq!(score.unmapped_labels, vec!["z".to_string()]);
        assert!((score.agreement - 0.9).abs() < 1e-6);
    }

    #[test]
    fn excerpts_are_found_only_near_their_turn() {
        let near = [TimedText {
            start_seconds: 1.0,
            end_seconds: 3.0,
            text: "Hello there, General!".into(),
        }];
        let score = score_transcript(&golden(), &near, 0.0, 0.8);
        assert_eq!(score.excerpts.len(), 2, "unquoted evidence is not scored");
        assert!(score.excerpts.iter().all(|excerpt| excerpt.found));

        let far = [TimedText {
            start_seconds: 8.0,
            end_seconds: 9.0,
            text: "hello there general".into(),
        }];
        assert_eq!(score_transcript(&golden(), &far, 3.0, 0.8).found_share, 0.0);
    }
}
