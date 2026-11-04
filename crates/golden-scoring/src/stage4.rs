//! Stage 4 of a golden run: proposition candidates → person attributions → score (belief-lab#63).
//!
//! 1. [`build_document`] turns the transcript and the diarized turns into the
//!    proposition-api extraction document: the transcript text, with one segment per speaker
//!    turn (UTF-8 byte range plus golden person id) and a byte → seconds timing table.
//! 2. [`map_candidates`] verifies each draft candidate's span against that text and maps it to
//!    attributions with the owner's stance map (2026-11-04).
//! 3. [`score_attributions`] matches claims to golden propositions (embedding cosine at least a
//!    fixed threshold and within ±30 s of the golden `approxSeconds`) and scores every expected
//!    attribution, the `expectedNoAttribution` rules and fabricated attributions.
//!
//! Everything here is pure and deterministic. Embeddings are computed by the caller and passed
//! in as a [`Similarities`] table, so tests use small precomputed tables.

use crate::{words, GoldenAttribution, GoldenExpectations, GoldenPerson, PredictedTurn, TimedText};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// A claim matches a golden attribution only within this distance of its `approxSeconds`.
pub const TIME_WINDOW_SECONDS: f64 = 30.0;
/// Unmatched claims this close below the threshold are listed as near-misses.
pub const NEAR_MISS_SIMILARITY: f64 = 0.05;
/// Claims above the threshold but outside the time window, up to this far, are near-misses.
pub const NEAR_MISS_SECONDS: f64 = 60.0;
/// The evidence quote and the golden quote must share this share of words (of the shorter).
pub const MIN_QUOTE_OVERLAP: f64 = 0.5;

// ---------------------------------------------------------------------------------------------
// Extraction document
// ---------------------------------------------------------------------------------------------

/// One speaker turn of the extraction document (proposition-api `ExtractionSegment`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSegment {
    pub id: String,
    /// Golden person id, absent when no mapped diarization label overlaps the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// The byte range of one transcript segment and its recorded media times.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedSpan {
    pub start: usize,
    pub end: usize,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageFourDocument {
    pub id: String,
    pub language: String,
    pub text: String,
    pub segments: Vec<DocumentSegment>,
    pub timing: Vec<TimedSpan>,
}

impl StageFourDocument {
    /// The proposition-api `PropositionExtractionInput` for this document.
    pub fn extraction_input(&self) -> Value {
        json!({
            "document": {
                "id": self.id,
                "kind": "transcript",
                "language": self.language,
                "text": self.text,
                "segments": self.segments,
            }
        })
    }

    /// Media time of a byte offset: linear within the transcript segment that contains it.
    /// Whisper reports times per segment, so this is exact at segment boundaries only.
    pub fn seconds_at(&self, byte: usize) -> Option<f64> {
        let span = self
            .timing
            .iter()
            .find(|span| byte >= span.start && byte < span.end)?;
        let share = (byte - span.start) as f64 / (span.end - span.start) as f64;
        Some(span.start_seconds + share * (span.end_seconds - span.start_seconds))
    }

    /// The speaker turn that contains the whole byte range.
    pub fn segment_containing(&self, start: usize, end: usize) -> Option<&DocumentSegment> {
        self.segments
            .iter()
            .find(|segment| start >= segment.start && end <= segment.end)
    }
}

/// Builds the extraction document. Each transcript segment gets the mapped speaker of the
/// diarization label that overlaps it longest; consecutive segments of one speaker form one turn.
/// Segments join with a space inside a turn and turns with a blank line.
pub fn build_document(
    id: &str,
    language: &str,
    transcript: &[TimedText],
    turns: &[PredictedTurn],
    label_mapping: &BTreeMap<String, String>,
) -> StageFourDocument {
    let mut text = String::new();
    let mut segments: Vec<DocumentSegment> = Vec::new();
    let mut timing = Vec::new();
    for piece in transcript {
        let words = piece.text.trim();
        if words.is_empty() {
            continue;
        }
        let speaker = speaker_of(piece, turns, label_mapping);
        let continues = segments
            .last()
            .is_some_and(|segment| segment.speaker == speaker);
        if continues {
            text.push(' ');
        } else if !text.is_empty() {
            text.push_str("\n\n");
        }
        let start = text.len();
        text.push_str(words);
        let end = text.len();
        timing.push(TimedSpan {
            start,
            end,
            start_seconds: piece.start_seconds,
            end_seconds: piece.end_seconds,
        });
        match segments.last_mut() {
            Some(segment) if continues => segment.end = end,
            _ => segments.push(DocumentSegment {
                id: format!("turn-{:03}", segments.len() + 1),
                speaker,
                start,
                end,
            }),
        }
    }
    StageFourDocument {
        id: id.to_owned(),
        language: language.to_owned(),
        text,
        segments,
        timing,
    }
}

fn speaker_of(
    piece: &TimedText,
    turns: &[PredictedTurn],
    label_mapping: &BTreeMap<String, String>,
) -> Option<String> {
    let mut overlap = BTreeMap::<&str, f64>::new();
    for turn in turns {
        let seconds =
            turn.end_seconds.min(piece.end_seconds) - turn.start_seconds.max(piece.start_seconds);
        if seconds > 0.0 {
            *overlap.entry(turn.speaker.as_str()).or_default() += seconds;
        }
    }
    let label = overlap.into_iter().max_by(|a, b| a.1.total_cmp(&b.1))?.0;
    label_mapping.get(label).cloned()
}

// ---------------------------------------------------------------------------------------------
// Candidates → attributions
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateEvidence {
    pub quote: String,
    pub start: usize,
    pub end: usize,
    #[serde(default)]
    pub segment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateReportingLayer {
    pub holder: String,
    pub cue: String,
    pub cue_span: ByteSpan,
}

/// The fields of a proposition-api candidate this scoring reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub canonical_text: String,
    #[serde(default)]
    pub evidence: Option<CandidateEvidence>,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    pub reporting: Vec<CandidateReportingLayer>,
    #[serde(default)]
    pub stance_cue: Option<String>,
    #[serde(default)]
    pub needs_refinement: Vec<String>,
}

/// One attribution derived from one candidate, shaped like a golden `expectedAttributions` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictedAttribution {
    pub candidate: usize,
    pub person: String,
    pub stance: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about_person: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributed_stance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// The draft marked the holder's stance as hedged (mapped to `no_stance`, never suspension).
    pub hedged: bool,
    pub seconds: f64,
    pub quote: String,
    pub span: ByteSpan,
    pub segment_id: String,
    pub canonical_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnattributedCandidate {
    pub candidate: usize,
    pub canonical_text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappedCandidates {
    pub candidates: usize,
    /// Candidates whose quote equals the transcript bytes and whose speaker equals its turn's.
    pub verified: usize,
    pub attributions: Vec<PredictedAttribution>,
    pub unattributed: Vec<UnattributedCandidate>,
    /// Reporting holders that name no golden person (e.g. "Sartre"), with their candidates.
    pub unresolved_holders: Vec<(usize, String)>,
}

/// Owner stance map (belief-lab#63, 2026-11-04) for the speaker or the innermost holder:
/// returns the stance and the `hedged` flag.
pub fn stance_from_cue(cue: &str) -> Option<(&'static str, bool)> {
    match cue {
        "asserted" => Some(("endorses", false)),
        "negated" => Some(("endorses_negation", false)),
        "questioned" => Some(("no_stance", false)),
        "hedged" => Some(("no_stance", true)),
        // Only supposed (proposition-api draft-v3, owner decision 2026-11-04): no stance.
        "conditional" => Some(("no_stance", false)),
        _ => None,
    }
}

const SELF_REFERENCES: [&str; 6] = ["i", "me", "myself", "we", "speaker", "the speaker"];

/// Resolves a free-text reporting holder to a golden person: first-person references are the
/// speaker; otherwise exactly one person whose id or display-name word (3+ letters, not shared
/// with another person) appears in the holder. Anything else stays unresolved.
pub fn resolve_holder(holder: &str, speaker: &str, people: &[GoldenPerson]) -> Option<String> {
    let lowered = holder.trim().to_lowercase();
    if SELF_REFERENCES.contains(&lowered.as_str()) {
        return Some(speaker.to_owned());
    }
    let holder_words: Vec<String> = words(holder)
        .into_iter()
        .map(|word| word.trim_end_matches("'s").to_owned())
        .collect();
    let names: Vec<(String, Vec<String>)> = people
        .iter()
        .map(|person| {
            let mut tokens = words(&person.display_name);
            tokens.push(person.id.to_lowercase());
            tokens.retain(|token| token.chars().count() >= 3);
            (person.id.clone(), tokens)
        })
        .collect();
    let shared = |token: &str| {
        names
            .iter()
            .filter(|(_, tokens)| tokens.iter().any(|candidate| candidate == token))
            .count()
            > 1
    };
    let mut hits: Vec<&String> = names
        .iter()
        .filter(|(_, tokens)| {
            tokens
                .iter()
                .any(|token| !shared(token) && holder_words.iter().any(|word| word == token))
        })
        .map(|(id, _)| id)
        .collect();
    hits.dedup();
    match hits.as_slice() {
        [one] => Some((*one).clone()),
        _ => None,
    }
}

/// Verifies every candidate against the document and maps it to attributions.
///
/// * Empty reporting (after dropping outer layers whose holder is the speaker): the speaker gets
///   the mapped stance as an `explicit_endorsement`.
/// * Reporting layers: every outer reporter that is a golden person (the speaker, then the outer
///   holders) gets `no_stance` / `reported_position` with `aboutPerson` = innermost holder,
///   `attributedStance` = the mapped stance and `via` = the reporter directly outside it (none for
///   the speaker); the innermost holder, when it is a golden person, gets
///   the mapped stance as a `reported_position` `via` the reporter directly outside it.
///
/// A candidate whose quote is not the exact transcript bytes, or whose speaker is not its turn's,
/// yields no attribution.
pub fn map_candidates(
    document: &StageFourDocument,
    people: &[GoldenPerson],
    candidates: &[Candidate],
) -> MappedCandidates {
    let mut mapped = MappedCandidates {
        candidates: candidates.len(),
        verified: 0,
        attributions: Vec::new(),
        unattributed: Vec::new(),
        unresolved_holders: Vec::new(),
    };
    for (index, candidate) in candidates.iter().enumerate() {
        let reject = |reason: &str| UnattributedCandidate {
            candidate: index,
            canonical_text: candidate.canonical_text.clone(),
            reason: reason.to_owned(),
        };
        let Some(evidence) = &candidate.evidence else {
            mapped.unattributed.push(reject("no_evidence"));
            continue;
        };
        if evidence.start >= evidence.end || evidence.quote.trim().is_empty() {
            mapped.unattributed.push(reject("empty_evidence"));
            continue;
        }
        if document.text.get(evidence.start..evidence.end) != Some(evidence.quote.as_str()) {
            mapped
                .unattributed
                .push(reject("quote_not_transcript_bytes"));
            continue;
        }
        let segment = document.segment_containing(evidence.start, evidence.end);
        let verified = segment.is_some_and(|segment| {
            evidence.segment_id.as_deref() == Some(segment.id.as_str())
                && segment.speaker.is_some()
                && segment.speaker == candidate.speaker
        });
        let (Some(segment), Some(speaker), true) = (segment, &candidate.speaker, verified) else {
            mapped
                .unattributed
                .push(reject("speaker_or_segment_mismatch"));
            continue;
        };
        mapped.verified += 1;
        let Some((stance, hedged)) = candidate.stance_cue.as_deref().and_then(stance_from_cue)
        else {
            mapped.unattributed.push(reject("unknown_stance_cue"));
            continue;
        };
        let Some(seconds) = document.seconds_at(evidence.start) else {
            mapped.unattributed.push(reject("no_timing"));
            continue;
        };
        let base = PredictedAttribution {
            candidate: index,
            person: speaker.clone(),
            stance: stance.to_owned(),
            kind: "explicit_endorsement".to_owned(),
            about_person: None,
            attributed_stance: None,
            via: None,
            hedged,
            seconds,
            quote: evidence.quote.clone(),
            span: ByteSpan {
                start: evidence.start,
                end: evidence.end,
            },
            segment_id: segment.id.clone(),
            canonical_text: candidate.canonical_text.clone(),
        };

        let holders: Vec<(String, Option<String>)> = candidate
            .reporting
            .iter()
            .map(|layer| {
                (
                    layer.holder.clone(),
                    resolve_holder(&layer.holder, speaker, people),
                )
            })
            .collect();
        let chain: Vec<&(String, Option<String>)> = holders
            .iter()
            .skip_while(|(_, person)| person.as_deref() == Some(speaker.as_str()))
            .collect();
        for (holder, person) in &chain {
            if person.is_none() {
                mapped.unresolved_holders.push((index, holder.clone()));
            }
        }
        let Some(((innermost_raw, innermost), outer)) = chain.split_last() else {
            mapped.attributions.push(base);
            continue;
        };
        let innermost_label = innermost
            .clone()
            .unwrap_or_else(|| format!("unresolved:{innermost_raw}"));
        let mut reporters: Vec<Option<String>> = vec![Some(speaker.clone())];
        reporters.extend(outer.iter().map(|(_, person)| person.clone()));
        for (position, reporter) in reporters.iter().enumerate() {
            let Some(reporter) = reporter else { continue };
            if Some(reporter) == innermost.as_ref() {
                continue;
            }
            // Each intermediate reporter keeps the link to the reporter directly outside it, so
            // the exported evidence chain stays connected (narrator → Copleston → Russell).
            let via = position
                .checked_sub(1)
                .and_then(|outer| reporters[outer].clone());
            mapped.attributions.push(PredictedAttribution {
                person: reporter.clone(),
                stance: "no_stance".to_owned(),
                kind: "reported_position".to_owned(),
                about_person: Some(innermost_label.clone()),
                attributed_stance: Some(stance.to_owned()),
                via,
                ..base.clone()
            });
        }
        if let Some(holder) = innermost {
            mapped.attributions.push(PredictedAttribution {
                person: holder.clone(),
                kind: "reported_position".to_owned(),
                via: reporters.last().cloned().flatten(),
                ..base
            });
        }
    }
    mapped
}

// ---------------------------------------------------------------------------------------------
// Matching and scoring
// ---------------------------------------------------------------------------------------------

/// Cosine similarity of each candidate's canonical text to each golden proposition text,
/// computed by the caller's embedding model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Similarities {
    pub model: String,
    pub revision: String,
    /// How the texts were embedded, e.g. `symmetric` (e5 `query: ` prefix on both sides).
    pub input_kind: String,
    /// `candidates[i][propositionId]` = cosine for candidate i.
    pub candidates: Vec<BTreeMap<String, f64>>,
}

impl Similarities {
    pub fn get(&self, candidate: usize, proposition: &str) -> f64 {
        self.candidates
            .get(candidate)
            .and_then(|row| row.get(proposition))
            .copied()
            .unwrap_or(f64::NEG_INFINITY)
    }
}

/// One NLI judgment of a claim (premise) against a golden proposition (hypothesis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NliLabel {
    Entailment,
    Neutral,
    Contradiction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NliJudgment {
    pub label: NliLabel,
    /// Model probabilities for entailment, neutral and contradiction, as the receipt.
    pub probabilities: [f64; 3],
}

/// NLI judgments for the claim/proposition pairs that pass the e5 match, computed by the caller's
/// pinned NLI model (owner decision, belief-lab#63, 2026-11-04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Polarity {
    pub model: String,
    pub revision: String,
    /// `candidates[i][propositionId]` for candidate i.
    pub candidates: Vec<BTreeMap<String, NliJudgment>>,
}

impl Polarity {
    pub fn label(&self, candidate: usize, proposition: &str) -> Option<NliLabel> {
        self.candidates
            .get(candidate)
            .and_then(|row| row.get(proposition))
            .map(|judgment| judgment.label)
    }

    /// Every pair at or above the similarity threshold needs a judgment: no silent default.
    pub fn check_complete(
        &self,
        golden: &GoldenExpectations,
        similarities: &Similarities,
        candidates: usize,
        threshold: f64,
    ) -> Result<(), String> {
        for candidate in 0..candidates {
            for proposition in &golden.propositions {
                if similarities.get(candidate, &proposition.id) >= threshold
                    && self.label(candidate, &proposition.id).is_none()
                {
                    return Err(format!(
                        "no NLI judgment for candidate {candidate} and `{}` from {}@{}",
                        proposition.id, self.model, self.revision
                    ));
                }
            }
        }
        Ok(())
    }
}

/// `endorses` ↔ `endorses_negation`; every other stance is unchanged.
pub fn flip_stance(stance: &str) -> String {
    match stance {
        "endorses" => "endorses_negation",
        "endorses_negation" => "endorses",
        other => other,
    }
    .to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringConfig {
    pub similarity_threshold: f64,
    pub time_window_seconds: f64,
    pub near_miss_similarity: f64,
    pub near_miss_seconds: f64,
    pub min_quote_overlap: f64,
}

impl ScoringConfig {
    pub fn with_threshold(similarity_threshold: f64) -> Self {
        Self {
            similarity_threshold,
            time_window_seconds: TIME_WINDOW_SECONDS,
            near_miss_similarity: NEAR_MISS_SIMILARITY,
            near_miss_seconds: NEAR_MISS_SECONDS,
            min_quote_overlap: MIN_QUOTE_OVERLAP,
        }
    }
}

/// One predicted attribution checked against one expected attribution.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionCheck {
    pub predicted: PredictedAttribution,
    pub similarity: f64,
    pub delta_seconds: f64,
    pub person_ok: bool,
    pub stance_ok: bool,
    pub kind_ok: bool,
    /// `via`, `aboutPerson` and `attributedStance` equal the golden entry's.
    pub reporter_ok: bool,
    pub span_ok: bool,
}

impl AttributionCheck {
    fn passed(&self) -> usize {
        [
            self.person_ok,
            self.stance_ok,
            self.kind_ok,
            self.reporter_ok,
            self.span_ok,
        ]
        .into_iter()
        .filter(|ok| *ok)
        .count()
    }

    fn all(&self) -> bool {
        self.passed() == 5
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedResult {
    pub expected: GoldenAttribution,
    /// `hit` (some matched attribution passes every check), `partial` (a claim matched but no
    /// attribution passes all checks), `miss` (no claim matched); for `no_stance_evidence`
    /// entries `held` or `violated`.
    pub status: String,
    /// Candidates whose claim matched this entry (similarity and time).
    pub matched_candidates: Vec<usize>,
    /// The matched attribution passing the most checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best: Option<AttributionCheck>,
    /// For `no_stance_evidence`: attributions that state a stance anyway.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<PredictedAttribution>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoAttributionResult {
    pub person: String,
    pub proposition: String,
    pub held: bool,
    /// Attributions of a stance to the person from their own words (no `via`).
    pub violations: Vec<PredictedAttribution>,
    /// Attributions to the person that come from someone's report (allowed).
    pub reported: Vec<PredictedAttribution>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fabrication {
    pub predicted: PredictedAttribution,
    pub proposition: String,
    pub similarity: f64,
    /// `wrong_stance`, `contradicts_no_stance` or `wrong_person`.
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearMiss {
    pub expected_index: usize,
    pub person: String,
    pub proposition: String,
    pub candidate: usize,
    pub canonical_text: String,
    pub similarity: f64,
    pub delta_seconds: f64,
    /// `similarity` (below the threshold by at most the margin), `time` (outside ±30 s) or
    /// `nli_neutral` (above the threshold, but NLI found neither entailment nor contradiction).
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionScore {
    pub config: ScoringConfig,
    /// The NLI model that checked polarity, or none for the cosine-only baseline.
    pub polarity_model: Option<String>,
    pub similarity_model: String,
    pub similarity_revision: String,
    pub candidates: usize,
    pub verified_candidates: usize,
    pub predicted_attributions: usize,
    pub unattributed: Vec<UnattributedCandidate>,
    pub unresolved_holders: Vec<(usize, String)>,
    /// Candidates matching no golden proposition above the threshold (golden is not exhaustive).
    pub candidates_matching_no_proposition: usize,
    pub expected: Vec<ExpectedResult>,
    pub hits: usize,
    pub partial: usize,
    pub misses: usize,
    pub no_attribution: Vec<NoAttributionResult>,
    pub fabricated: Vec<Fabrication>,
    pub near_misses: Vec<NearMiss>,
}

fn attributed(stance: &str) -> bool {
    matches!(
        stance,
        "endorses" | "endorses_negation" | "suspends_judgment"
    )
}

/// Share of the shorter quote's words found in the other (fragments split by `...`).
pub fn quote_overlap(golden_quote: &str, quote: &str) -> f64 {
    let golden: Vec<String> = golden_quote.split("...").flat_map(words).collect();
    let predicted = words(quote);
    if golden.is_empty() || predicted.is_empty() {
        return 0.0;
    }
    let golden_in = golden
        .iter()
        .filter(|word| predicted.contains(word))
        .count() as f64;
    let predicted_in = predicted
        .iter()
        .filter(|word| golden.contains(word))
        .count() as f64;
    (golden_in / golden.len() as f64).max(predicted_in / predicted.len() as f64)
}

/// Scores without a polarity check: the e5 match alone decides (the 2026-11-04 baseline).
pub fn score_attributions(
    golden: &GoldenExpectations,
    mapped: &MappedCandidates,
    similarities: &Similarities,
    config: ScoringConfig,
) -> AttributionScore {
    score_with_polarity(golden, mapped, similarities, None, config)
        .expect("scoring without a polarity table cannot fail")
}

/// Scores with the owner's polarity rule (belief-lab#63, 2026-11-04) when `polarity` is given:
/// a claim that passes the e5 match is checked against the golden proposition with NLI;
/// `entailment` keeps its stance, `contradiction` flips it (`endorses` ↔ `endorses_negation`),
/// `neutral` is no match. Fails when the table lacks a pair at or above the threshold.
pub fn score_with_polarity(
    golden: &GoldenExpectations,
    mapped: &MappedCandidates,
    similarities: &Similarities,
    polarity: Option<&Polarity>,
    config: ScoringConfig,
) -> Result<AttributionScore, String> {
    let threshold = config.similarity_threshold;
    if let Some(polarity) = polarity {
        polarity.check_complete(golden, similarities, mapped.candidates, threshold)?;
    }
    let label = |candidate: usize, proposition: &str| {
        polarity.and_then(|polarity| polarity.label(candidate, proposition))
    };
    // Similarity as the matcher sees it: a neutral NLI judgment is no match.
    let similarities_get = |candidate: usize, proposition: &str| {
        if label(candidate, proposition) == Some(NliLabel::Neutral) {
            f64::NEG_INFINITY
        } else {
            similarities.get(candidate, proposition)
        }
    };
    // The attribution's stance relative to one golden proposition.
    let relative = |predicted: &PredictedAttribution, proposition: &str| {
        let mut predicted = predicted.clone();
        if label(predicted.candidate, proposition) == Some(NliLabel::Contradiction) {
            predicted.stance = flip_stance(&predicted.stance);
            predicted.attributed_stance = predicted.attributed_stance.as_deref().map(flip_stance);
        }
        predicted
    };
    let candidate_seconds: BTreeMap<usize, f64> = mapped
        .attributions
        .iter()
        .map(|predicted| (predicted.candidate, predicted.seconds))
        .collect();
    let mut expected_results = Vec::new();
    let mut near_misses = Vec::new();
    for (expected_index, expected) in golden.expected_attributions.iter().enumerate() {
        if expected.stance == "no_stance_evidence" {
            let violations: Vec<PredictedAttribution> = mapped
                .attributions
                .iter()
                .map(|predicted| relative(predicted, &expected.proposition))
                .filter(|predicted| {
                    predicted.person == expected.person
                        && attributed(&predicted.stance)
                        && similarities_get(predicted.candidate, &expected.proposition) >= threshold
                })
                .collect();
            expected_results.push(ExpectedResult {
                expected: expected.clone(),
                status: if violations.is_empty() {
                    "held"
                } else {
                    "violated"
                }
                .to_owned(),
                matched_candidates: Vec::new(),
                best: None,
                violations,
            });
            continue;
        }
        let Some(approx) = expected.approx_seconds else {
            continue;
        };
        let mut matched = Vec::new();
        for (&candidate, &seconds) in &candidate_seconds {
            let similarity = similarities_get(candidate, &expected.proposition);
            let delta = (seconds - approx).abs();
            if similarity >= threshold && delta <= config.time_window_seconds {
                matched.push(candidate);
                continue;
            }
            let raw = similarities.get(candidate, &expected.proposition);
            let reason = if label(candidate, &expected.proposition) == Some(NliLabel::Neutral)
                && raw >= threshold
                && delta <= config.near_miss_seconds
            {
                Some("nli_neutral")
            } else if similarity < threshold
                && similarity >= threshold - config.near_miss_similarity
                && delta <= config.near_miss_seconds
            {
                Some("similarity")
            } else if similarity >= threshold && delta <= config.near_miss_seconds {
                Some("time")
            } else {
                None
            };
            if let Some(reason) = reason {
                let canonical_text = mapped
                    .attributions
                    .iter()
                    .find(|predicted| predicted.candidate == candidate)
                    .map(|predicted| predicted.canonical_text.clone())
                    .unwrap_or_default();
                near_misses.push(NearMiss {
                    expected_index,
                    person: expected.person.clone(),
                    proposition: expected.proposition.clone(),
                    candidate,
                    canonical_text,
                    similarity: raw,
                    delta_seconds: seconds - approx,
                    reason: reason.to_owned(),
                });
            }
        }
        let best = mapped
            .attributions
            .iter()
            .filter(|predicted| matched.contains(&predicted.candidate))
            .map(|predicted| relative(predicted, &expected.proposition))
            .map(|predicted| AttributionCheck {
                similarity: similarities_get(predicted.candidate, &expected.proposition),
                delta_seconds: predicted.seconds - approx,
                person_ok: predicted.person == expected.person,
                stance_ok: predicted.stance == expected.stance,
                kind_ok: Some(&predicted.kind) == expected.kind.as_ref(),
                reporter_ok: predicted.via == expected.via
                    && predicted.about_person == expected.about_person
                    && (expected.attributed_stance.is_none()
                        || predicted.attributed_stance == expected.attributed_stance),
                span_ok: expected.quote.as_deref().is_some_and(|quote| {
                    quote_overlap(quote, &predicted.quote) >= config.min_quote_overlap
                }),
                predicted: predicted.clone(),
            })
            .max_by(|a, b| {
                (a.person_ok, a.passed())
                    .cmp(&(b.person_ok, b.passed()))
                    .then(b.delta_seconds.abs().total_cmp(&a.delta_seconds.abs()))
            });
        let status = match (&best, matched.is_empty()) {
            (_, true) => "miss",
            (Some(best), false) if best.all() => "hit",
            _ => "partial",
        };
        expected_results.push(ExpectedResult {
            expected: expected.clone(),
            status: status.to_owned(),
            matched_candidates: matched,
            best,
            violations: Vec::new(),
        });
    }

    let no_attribution = golden
        .expected_no_attribution
        .iter()
        .map(|rule| {
            let about: Vec<PredictedAttribution> = mapped
                .attributions
                .iter()
                .map(|predicted| relative(predicted, &rule.proposition))
                .filter(|predicted| {
                    predicted.person == rule.person
                        && attributed(&predicted.stance)
                        && similarities_get(predicted.candidate, &rule.proposition) >= threshold
                })
                .collect();
            let violations: Vec<PredictedAttribution> = about
                .iter()
                .filter(|predicted| predicted.via.is_none())
                .cloned()
                .collect();
            NoAttributionResult {
                person: rule.person.clone(),
                proposition: rule.proposition.clone(),
                held: violations.is_empty(),
                violations,
                reported: about
                    .iter()
                    .filter(|predicted| predicted.via.is_some())
                    .cloned()
                    .collect(),
            }
        })
        .collect();

    let mut fabricated = Vec::new();
    for unrelated in &mapped.attributions {
        for proposition in &golden.propositions {
            let predicted = &relative(unrelated, &proposition.id);
            if !attributed(&predicted.stance) {
                continue;
            }
            let similarity = similarities_get(predicted.candidate, &proposition.id);
            if similarity < threshold {
                continue;
            }
            let on_proposition: Vec<&GoldenAttribution> = golden
                .expected_attributions
                .iter()
                .filter(|expected| expected.proposition == proposition.id)
                .collect();
            let in_window = on_proposition.iter().any(|expected| {
                expected.approx_seconds.is_some_and(|approx| {
                    (predicted.seconds - approx).abs() <= config.time_window_seconds
                })
            });
            let ruled_out =
                golden.expected_no_attribution.iter().any(|rule| {
                    rule.person == predicted.person && rule.proposition == proposition.id
                }) && predicted.via.is_none();
            let same_person: Vec<&&GoldenAttribution> = on_proposition
                .iter()
                .filter(|expected| expected.person == predicted.person)
                .collect();
            if !(in_window || ruled_out || !same_person.is_empty()) {
                continue;
            }
            // An own-words prohibition wins over a golden entry with the same stance (which is
            // then a reported position, not the person's own words).
            if !ruled_out
                && same_person
                    .iter()
                    .any(|expected| expected.stance == predicted.stance)
            {
                continue;
            }
            let reason = if ruled_out
                || same_person.iter().any(|expected| {
                    matches!(expected.stance.as_str(), "no_stance" | "no_stance_evidence")
                }) {
                "contradicts_no_stance"
            } else if same_person.is_empty() {
                "wrong_person"
            } else {
                "wrong_stance"
            };
            fabricated.push(Fabrication {
                predicted: predicted.clone(),
                proposition: proposition.id.clone(),
                similarity,
                reason: reason.to_owned(),
            });
        }
    }

    let candidates_matching_no_proposition = (0..mapped.candidates)
        .filter(|&candidate| {
            golden
                .propositions
                .iter()
                .all(|proposition| similarities_get(candidate, &proposition.id) < threshold)
        })
        .count();
    let count = |status: &str| {
        expected_results
            .iter()
            .filter(|result| result.status == status)
            .count()
    };
    Ok(AttributionScore {
        config,
        polarity_model: polarity
            .map(|polarity| format!("{}@{}", polarity.model, polarity.revision)),
        similarity_model: similarities.model.clone(),
        similarity_revision: similarities.revision.clone(),
        candidates: mapped.candidates,
        verified_candidates: mapped.verified,
        predicted_attributions: mapped.attributions.len(),
        unattributed: mapped.unattributed.clone(),
        unresolved_holders: mapped.unresolved_holders.clone(),
        candidates_matching_no_proposition,
        hits: count("hit"),
        partial: count("partial"),
        misses: count("miss"),
        expected: expected_results,
        no_attribution,
        fabricated,
        near_misses,
    })
}

/// A short human-readable summary of an attribution score.
pub fn summary_markdown(score: &AttributionScore) -> String {
    let mut out = String::new();
    let config = score.config;
    out.push_str(&format!(
        "# Stage 4: attributions\n\nMatching: {} cosine >= {:.2} and |claim time - approxSeconds| <= {:.0} s.\n\n",
        score.similarity_model, config.similarity_threshold, config.time_window_seconds
    ));
    out.push_str(&format!(
        "- Candidates: {} ({} verified against the transcript bytes and speaker turns), {} attributions, {} candidates unattributed, {} match no golden proposition.\n",
        score.candidates,
        score.verified_candidates,
        score.predicted_attributions,
        score.unattributed.len(),
        score.candidates_matching_no_proposition
    ));
    let timed = score.hits + score.partial + score.misses;
    out.push_str(&format!(
        "- Expected attributions (timed): {} hit, {} partial, {} miss of {timed}.\n",
        score.hits, score.partial, score.misses
    ));
    for result in score
        .expected
        .iter()
        .filter(|result| matches!(result.status.as_str(), "held" | "violated"))
    {
        out.push_str(&format!(
            "- No stance evidence {} / {}: {}.\n",
            result.expected.person, result.expected.proposition, result.status
        ));
    }
    for rule in &score.no_attribution {
        out.push_str(&format!(
            "- expectedNoAttribution {} / {}: {} ({} own-words violations, {} reported).\n",
            rule.person,
            rule.proposition,
            if rule.held { "held" } else { "VIOLATED" },
            rule.violations.len(),
            rule.reported.len()
        ));
    }
    out.push_str(&format!(
        "- Fabricated attributions: {}. Near-misses: {}.\n\n",
        score.fabricated.len(),
        score.near_misses.len()
    ));
    out.push_str("| # | person | proposition | stance | kind | status | sim | Δt (s) | failed checks |\n|---|---|---|---|---|---|---|---|---|\n");
    for (index, result) in score.expected.iter().enumerate() {
        let expected = &result.expected;
        let (similarity, delta, failed) = match &result.best {
            Some(best) => {
                let failed: Vec<&str> = [
                    ("person", best.person_ok),
                    ("stance", best.stance_ok),
                    ("kind", best.kind_ok),
                    ("reporter", best.reporter_ok),
                    ("span", best.span_ok),
                ]
                .into_iter()
                .filter(|(_, ok)| !ok)
                .map(|(name, _)| name)
                .collect();
                (
                    format!("{:.3}", best.similarity),
                    format!("{:+.0}", best.delta_seconds),
                    failed.join(", "),
                )
            }
            None => ("-".into(), "-".into(), String::new()),
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {similarity} | {delta} | {failed} |\n",
            index + 1,
            expected.person,
            expected.proposition,
            expected.stance,
            expected.kind.as_deref().unwrap_or("-"),
            result.status,
        ));
    }
    if !score.fabricated.is_empty() {
        out.push_str("\n## Fabricated\n\n");
        for fabrication in &score.fabricated {
            out.push_str(&format!(
                "- {} {} `{}` ({}, sim {:.3}, {:.0} s): \"{}\"\n",
                fabrication.predicted.person,
                fabrication.predicted.stance,
                fabrication.proposition,
                fabrication.reason,
                fabrication.similarity,
                fabrication.predicted.seconds,
                fabrication.predicted.quote
            ));
        }
    }
    if !score.near_misses.is_empty() {
        out.push_str("\n## Near-misses\n\n");
        for miss in &score.near_misses {
            out.push_str(&format!(
                "- #{} {} / {}: candidate {} ({}, sim {:.3}, Δt {:+.0} s) \"{}\"\n",
                miss.expected_index + 1,
                miss.person,
                miss.proposition,
                miss.candidate,
                miss.reason,
                miss.similarity,
                miss.delta_seconds,
                miss.canonical_text
            ));
        }
    }
    out
}
