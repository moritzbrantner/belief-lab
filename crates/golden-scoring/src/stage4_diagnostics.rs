//! Read-only, case-level diagnostics for the stage-4 golden attribution scorer.
//!
//! These are observations of scoring gates, not claims about the underlying semantic cause of a
//! model error. The same saved candidates, cosine table and NLI judgments always yield the same
//! diagnostic report; no model execution or producer-owned evidence is changed here.

use crate::stage4::{
    quote_overlap, AttributionScore, ByteSpan, Candidate, CandidateReportingLayer, Fabrication,
    MappedCandidates, NliJudgment, Polarity, PredictedAttribution, ScoringConfig, Similarities,
    StageFourDocument,
};
use crate::GoldenAttribution;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateTrace {
    pub candidate: usize,
    pub canonical_text: String,
    pub source_quote: Option<String>,
    pub source_span: Option<ByteSpan>,
    pub source_segment_id: Option<String>,
    pub speaker: Option<String>,
    pub seconds: Option<f64>,
    pub stance_cue: Option<String>,
    pub reporting: Vec<CandidateReportingLayer>,
    pub needs_refinement: Vec<String>,
    pub mapped_attributions: Vec<PredictedAttribution>,
    pub mapping_error: Option<String>,
    pub similarity: Option<f64>,
    pub delta_seconds: Option<f64>,
    pub quote_overlap: Option<f64>,
    pub nli: Option<NliJudgment>,
    /// Observed matcher gates, not an inferred model-error root cause.
    pub failed_gates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedDiagnostic {
    /// Zero-based index in the owner-approved expectedAttributions list.
    pub expected_index: usize,
    pub expected: GoldenAttribution,
    pub status: String,
    pub observed_blockers: Vec<String>,
    /// A bounded union of matches, near-misses, nearest times, closest source quotes and best
    /// cosine matches. A missing candidate is not silently called an extraction-model failure.
    pub candidate_traces: Vec<CandidateTrace>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricationDiagnostic {
    pub fabrication: Fabrication,
    pub candidate_trace: CandidateTrace,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoAttributionDiagnostic {
    pub person: String,
    pub proposition: String,
    pub held: bool,
    pub violations: Vec<CandidateTrace>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageFourDiagnostics {
    pub schema: &'static str,
    pub schema_version: u32,
    pub document_id: String,
    pub config: ScoringConfig,
    pub similarity_model: String,
    pub similarity_revision: String,
    pub polarity_model: Option<String>,
    pub polarity_revision: Option<String>,
    pub expected: Vec<ExpectedDiagnostic>,
    pub fabricated: Vec<FabricationDiagnostic>,
    pub no_attribution: Vec<NoAttributionDiagnostic>,
}

fn trace(
    document: &StageFourDocument,
    candidates: &[Candidate],
    mapped: &MappedCandidates,
    similarities: &Similarities,
    polarity: Option<&Polarity>,
    config: ScoringConfig,
    index: usize,
    expected: Option<&GoldenAttribution>,
    proposition: &str,
) -> CandidateTrace {
    let candidate = &candidates[index];
    let seconds = candidate
        .evidence
        .as_ref()
        .and_then(|evidence| document.seconds_at(evidence.start));
    let similarity = similarities
        .candidates
        .get(index)
        .and_then(|row| row.get(proposition))
        .copied()
        .filter(|value| value.is_finite());
    let nli = polarity
        .and_then(|table| table.candidates.get(index))
        .and_then(|row| row.get(proposition))
        .cloned();
    let mapping_error = mapped
        .unattributed
        .iter()
        .find(|item| item.candidate == index)
        .map(|item| item.reason.clone());
    let mapped_attributions: Vec<_> = mapped
        .attributions
        .iter()
        .filter(|attribution| attribution.candidate == index)
        .cloned()
        .collect();
    let delta_seconds = expected
        .and_then(|entry| entry.approx_seconds)
        .and_then(|approx| seconds.map(|seconds| seconds - approx));
    let quote_overlap = expected
        .and_then(|entry| entry.quote.as_deref())
        .and_then(|quote| {
            candidate
                .evidence
                .as_ref()
                .map(|evidence| quote_overlap(quote, &evidence.quote))
        });
    let mut failed_gates = Vec::new();
    if let Some(reason) = &mapping_error {
        failed_gates.push(format!("mapping:{reason}"));
    }
    if let Some(delta) = delta_seconds {
        if delta.abs() > config.time_window_seconds {
            failed_gates.push("time_window".into());
        }
    }
    if similarity.is_none_or(|value| value < config.similarity_threshold) {
        failed_gates.push("similarity_threshold".into());
    }
    if nli
        .as_ref()
        .is_some_and(|judgment| judgment.label == crate::stage4::NliLabel::Neutral)
    {
        failed_gates.push("nli_neutral".into());
    }
    if quote_overlap.is_some_and(|overlap| overlap < config.min_quote_overlap) {
        failed_gates.push("quote_overlap".into());
    }
    CandidateTrace {
        candidate: index,
        canonical_text: candidate.canonical_text.clone(),
        source_quote: candidate
            .evidence
            .as_ref()
            .map(|evidence| evidence.quote.clone()),
        source_span: candidate.evidence.as_ref().map(|evidence| ByteSpan {
            start: evidence.start,
            end: evidence.end,
        }),
        source_segment_id: candidate
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.segment_id.clone()),
        speaker: candidate.speaker.clone(),
        seconds,
        stance_cue: candidate.stance_cue.clone(),
        reporting: candidate.reporting.clone(),
        needs_refinement: candidate.needs_refinement.clone(),
        mapped_attributions,
        mapping_error,
        similarity,
        delta_seconds,
        quote_overlap,
        nli,
        failed_gates,
    }
}

fn candidates_for_expected(
    expected_index: usize,
    expected: &GoldenAttribution,
    score: &AttributionScore,
    document: &StageFourDocument,
    candidates: &[Candidate],
    similarities: &Similarities,
) -> Vec<usize> {
    let mut selected = BTreeSet::new();
    selected.extend(
        score.expected[expected_index]
            .matched_candidates
            .iter()
            .copied(),
    );
    // A no-stance-evidence expectation has no timed matches. Its violations are nevertheless
    // essential to the case diagnostic and must not be lost behind the top-three cosine limit.
    selected.extend(
        score.expected[expected_index]
            .violations
            .iter()
            .map(|violation| violation.candidate),
    );
    selected.extend(
        score
            .near_misses
            .iter()
            .filter(|near| near.expected_index == expected_index)
            .map(|near| near.candidate),
    );

    // Independent views prevent an attractive but unrelated high cosine from hiding a candidate
    // close to the actual speech, and retain source-quote candidates that mapping rejected.
    let mut by_time: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let approx = expected.approx_seconds?;
            let seconds = document.seconds_at(candidate.evidence.as_ref()?.start)?;
            Some((index, (seconds - approx).abs()))
        })
        .collect();
    by_time.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    selected.extend(by_time.into_iter().take(3).map(|(index, _)| index));

    let mut by_quote: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let overlap = quote_overlap(
                expected.quote.as_deref()?,
                &candidate.evidence.as_ref()?.quote,
            );
            (overlap > 0.0).then_some((index, overlap))
        })
        .collect();
    by_quote.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    selected.extend(by_quote.into_iter().take(3).map(|(index, _)| index));

    let mut by_cosine: Vec<(usize, f64)> = similarities
        .candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| *index < candidates.len())
        .filter_map(|(index, row)| row.get(&expected.proposition).map(|value| (index, *value)))
        .filter(|(_, similarity)| similarity.is_finite())
        .collect();
    by_cosine.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    selected.extend(by_cosine.into_iter().take(3).map(|(index, _)| index));
    selected.into_iter().collect()
}

fn blockers(
    expected_index: usize,
    score: &AttributionScore,
    document: &StageFourDocument,
    candidates: &[Candidate],
    mapped: &MappedCandidates,
) -> Vec<String> {
    let result = &score.expected[expected_index];
    if matches!(result.status.as_str(), "hit" | "held") {
        return Vec::new();
    }
    if result.status == "violated" {
        return vec!["unexpected_stance_evidence".into()];
    }
    if let Some(best) = &result.best {
        return [
            ("person", best.person_ok),
            ("stance", best.stance_ok),
            ("kind", best.kind_ok),
            ("reporter", best.reporter_ok),
            ("quote_overlap", best.span_ok),
        ]
        .into_iter()
        .filter(|(_, passed)| !passed)
        .map(|(name, _)| name.to_owned())
        .collect();
    }
    let mut observed = Vec::new();
    for near in score
        .near_misses
        .iter()
        .filter(|near| near.expected_index == expected_index)
    {
        let reason = match near.reason.as_str() {
            "similarity" => "similarity_threshold",
            "time" => "time_window",
            "nli_neutral" => "nli_neutral",
            other => other,
        };
        if !observed.iter().any(|existing| existing == reason) {
            observed.push(reason.to_owned());
        }
    }
    if !observed.is_empty() {
        return observed;
    }
    let expected = &result.expected;
    let source_in_window = candidates.iter().any(|candidate| {
        expected.approx_seconds.is_some_and(|approx| {
            candidate
                .evidence
                .as_ref()
                .and_then(|evidence| document.seconds_at(evidence.start))
                .is_some_and(|seconds| (seconds - approx).abs() <= score.config.time_window_seconds)
        })
    });
    let mapped_in_window = mapped.attributions.iter().any(|attribution| {
        expected.approx_seconds.is_some_and(|approx| {
            (attribution.seconds - approx).abs() <= score.config.time_window_seconds
        })
    });
    vec![if !source_in_window {
        "no_source_candidate_in_time_window"
    } else if !mapped_in_window {
        "no_grounded_attribution_in_time_window"
    } else {
        "no_candidate_passed_matching_gates"
    }
    .into()]
}

/// Explain every approved expectation, forbidden attribution and fabrication using the exact
/// observations already scored. This function never changes the score or invokes a model.
pub fn diagnose_stage4(
    document: &StageFourDocument,
    candidates: &[Candidate],
    mapped: &MappedCandidates,
    similarities: &Similarities,
    polarity: Option<&Polarity>,
    score: &AttributionScore,
) -> StageFourDiagnostics {
    let expected = score
        .expected
        .iter()
        .enumerate()
        .map(|(index, result)| ExpectedDiagnostic {
            expected_index: index,
            expected: result.expected.clone(),
            status: result.status.clone(),
            observed_blockers: blockers(index, score, document, candidates, mapped),
            candidate_traces: candidates_for_expected(
                index,
                &result.expected,
                score,
                document,
                candidates,
                similarities,
            )
            .into_iter()
            .map(|candidate| {
                trace(
                    document,
                    candidates,
                    mapped,
                    similarities,
                    polarity,
                    score.config,
                    candidate,
                    Some(&result.expected),
                    &result.expected.proposition,
                )
            })
            .collect(),
        })
        .collect();
    let fabricated = score
        .fabricated
        .iter()
        .map(|fabrication| FabricationDiagnostic {
            fabrication: fabrication.clone(),
            candidate_trace: trace(
                document,
                candidates,
                mapped,
                similarities,
                polarity,
                score.config,
                fabrication.predicted.candidate,
                None,
                &fabrication.proposition,
            ),
        })
        .collect();
    let no_attribution = score
        .no_attribution
        .iter()
        .map(|result| NoAttributionDiagnostic {
            person: result.person.clone(),
            proposition: result.proposition.clone(),
            held: result.held,
            violations: result
                .violations
                .iter()
                .map(|violation| {
                    trace(
                        document,
                        candidates,
                        mapped,
                        similarities,
                        polarity,
                        score.config,
                        violation.candidate,
                        None,
                        &result.proposition,
                    )
                })
                .collect(),
        })
        .collect();
    StageFourDiagnostics {
        schema: "belief_stage4_diagnostics",
        schema_version: 1,
        document_id: document.id.clone(),
        config: score.config,
        similarity_model: similarities.model.clone(),
        similarity_revision: similarities.revision.clone(),
        polarity_model: polarity.map(|table| table.model.clone()),
        polarity_revision: polarity.map(|table| table.revision.clone()),
        expected,
        fabricated,
        no_attribution,
    }
}

/// A review-friendly companion to the complete machine-readable traces.
pub fn diagnostics_markdown(report: &StageFourDiagnostics) -> String {
    let mut out = format!(
        "# Stage 4: attribution diagnostics\n\nDocument: `{}`. Observed scorer gates only; model-error causes require review.\n\n",
        report.document_id
    );
    let hits = report
        .expected
        .iter()
        .filter(|result| result.status == "hit")
        .count();
    let timed = report
        .expected
        .iter()
        .filter(|result| !matches!(result.status.as_str(), "held" | "violated"))
        .count();
    out.push_str(&format!(
        "Timed expectations: {hits}/{timed} hits. Fabricated: {}. Forbidden own-words rules: {}.\n\n",
        report.fabricated.len(),
        report.no_attribution.len()
    ));
    for result in &report.expected {
        out.push_str(&format!(
            "## Expected #{} — {} / {} ({})\n\nStatus: **{}**. Observed blockers: {}.\n\n",
            result.expected_index + 1,
            result.expected.person,
            result.expected.proposition,
            result.expected.stance,
            result.status,
            if result.observed_blockers.is_empty() {
                "none".to_owned()
            } else {
                result.observed_blockers.join(", ")
            }
        ));
        if let Some(quote) = &result.expected.quote {
            out.push_str(&format!("Expected source quote: “{quote}”\n\n"));
        }
        for candidate in &result.candidate_traces {
            let nli = candidate
                .nli
                .as_ref()
                .map(|judgment| format!("{:?} {:?}", judgment.label, judgment.probabilities))
                .unwrap_or_else(|| "not measured".into());
            let quote = candidate
                .source_quote
                .as_deref()
                .unwrap_or("<no evidence>")
                .replace('\n', " ")
                .replace('\r', " ");
            let seconds = candidate
                .seconds
                .map(|value| format!("{value:.0}"))
                .unwrap_or_else(|| "unknown".into());
            let cosine = candidate
                .similarity
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "unavailable".into());
            let attribution = candidate
                .mapped_attributions
                .iter()
                .map(|item| format!("{}:{}:{}", item.person, item.stance, item.kind))
                .collect::<Vec<_>>()
                .join(", ");
            let mapping = candidate
                .mapping_error
                .as_deref()
                .map(|reason| format!("rejected ({reason})"))
                .unwrap_or(attribution);
            out.push_str(&format!(
                "- Candidate #{}: speaker {}, {} s, cosine {}, NLI {}; mapping [{}], gates [{}]; claim: “{}”; quote: “{}”\n",
                candidate.candidate,
                candidate.speaker.as_deref().unwrap_or("unknown"),
                seconds,
                cosine,
                nli,
                mapping,
                candidate.failed_gates.join(", "),
                candidate.canonical_text,
                quote.chars().take(160).collect::<String>()
            ));
        }
        out.push('\n');
    }
    out.push_str("## Fabricated attributions\n\n");
    for result in &report.fabricated {
        out.push_str(&format!(
            "- Candidate #{}: {} {} `{}` — {}. Source: “{}”\n",
            result.candidate_trace.candidate,
            result.fabrication.predicted.person,
            result.fabrication.predicted.stance,
            result.fabrication.proposition,
            result.fabrication.reason,
            result
                .fabrication
                .predicted
                .quote
                .replace('\n', " ")
                .replace('\r', " ")
        ));
    }
    out.push_str("\n## Forbidden own-words attributions\n\n");
    for result in &report.no_attribution {
        out.push_str(&format!(
            "- {} / {}: {} ({} violations)\n",
            result.person,
            result.proposition,
            if result.held { "held" } else { "VIOLATED" },
            result.violations.len()
        ));
    }
    out
}
