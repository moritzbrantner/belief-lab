//! Opt-in stage 4 of the Russell–Copleston golden test (belief-lab#63): propositions,
//! attributions and stance.
//!
//! Inputs, all under `target/golden/russell-copleston-1948/`:
//! - `stage4-document.json` from the stage 1–3 test (transcript text, speaker turns, timing);
//! - `stage4-candidates.json` from `stage4/extract.ts` (proposition-api `llama-cpp` draft, PR #12).
//!
//! Claims are matched to golden propositions with multilingual-e5-base (nlp-stack#188 preset) from
//! the local `model-runtime` bundle (`TEXT_EMBEDDINGS_MODEL_DIR`, default
//! `~/.cache/model-runtime`; never downloaded here). Writes `attributions-report.json`,
//! `attributions-summary.md`, `similarities.json`, the belief-lab evidence batch and
//! investigation, belief-lab's own report over them, and updates `report.json`.
use belief_cli::attribution::{compare, ComparisonInput};
use golden_scoring::interchange::{belief_lab_export, ExportProvenance};
use golden_scoring::stage4::{
    map_candidates, score_attributions, score_with_polarity, summary_markdown, Candidate,
    NliJudgment, NliLabel, Polarity, ScoringConfig, Similarities, StageFourDocument,
};
use golden_scoring::stage4_diagnostics::{diagnose_stage4, diagnostics_markdown};
use golden_scoring::GoldenExpectations;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use text_classification::{load_nli_preset, NliPreset, NliPresetLoadOptions};
use text_embeddings::{
    load_embedding_preset, EmbeddingPresetLoadOptions, MultilingualEmbeddingPreset,
    TextEmbeddingBackend,
};

/// e5 cosine a claim needs to match a golden proposition, chosen from the polarity threshold
/// sweep of the draft-v3 run (README "Stage 4 calibration"): the highest threshold that keeps the
/// most hits, which also gives the fewest fabrications.
const SIMILARITY_THRESHOLD: f64 = 0.92;
/// Lowest threshold of the calibration sweep; NLI judges every pair at or above it.
const SWEEP_MIN: f64 = 0.80;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cosine(left: &[f32], right: &[f32]) -> f64 {
    let dot: f64 = left
        .iter()
        .zip(right)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum();
    let norm = |values: &[f32]| {
        values
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    dot / (norm(left) * norm(right))
}

#[test]
#[ignore = "needs the stage 1-3 outputs, the stage-4 draft candidates and the cached e5 bundle"]
fn stage4_attributions_match_the_golden_file() {
    let out = root().join("target/golden/russell-copleston-1948");
    let read = |name: &str| -> Vec<u8> {
        std::fs::read(out.join(name)).unwrap_or_else(|error| {
            panic!(
                "{name} is missing ({error}); run the stage 1-3 test and stage4/extract.ts first"
            )
        })
    };
    let golden_json = std::fs::read_to_string(
        root().join("fixtures/golden/russell-copleston-1948/expected-beliefs.json"),
    )
    .unwrap();
    let golden = GoldenExpectations::parse(&golden_json).unwrap();
    assert!(golden.is_approved());

    let document_bytes = read("stage4-document.json");
    let document: StageFourDocument = serde_json::from_slice(&document_bytes).unwrap();
    let extraction: Value = serde_json::from_slice(&read("stage4-candidates.json")).unwrap();
    assert_eq!(
        extraction["documentSha256"],
        sha256_hex(&document_bytes),
        "candidates were drafted from a different document"
    );
    let candidates: Vec<Candidate> =
        serde_json::from_value(extraction["candidates"].clone()).unwrap();

    // Embeddings: symmetric e5 input (`query: ` on both sides), L2-normalized by the preset.
    let started = Instant::now();
    let mut options = EmbeddingPresetLoadOptions::default();
    if std::env::var_os("TEXT_EMBEDDINGS_MODEL_DIR").is_none() {
        options.bundle_root =
            PathBuf::from(std::env::var("HOME").unwrap()).join(".cache/model-runtime");
    }
    let preset = MultilingualEmbeddingPreset::MultilingualE5Base;
    let embedder = load_embedding_preset(preset, &options).expect("multilingual-e5-base bundle");
    let embed = |texts: &[&str]| -> Vec<Vec<f32>> {
        texts
            .chunks(16)
            .flat_map(|chunk| embedder.embed_batch(chunk).unwrap())
            .map(|vector| vector.as_slice().to_vec())
            .collect()
    };
    let proposition_texts: Vec<&str> = golden
        .propositions
        .iter()
        .map(|p| p.text.as_str())
        .collect();
    let proposition_vectors = embed(&proposition_texts);
    let candidate_texts: Vec<&str> = candidates
        .iter()
        .map(|c| c.canonical_text.as_str())
        .collect();
    let candidate_vectors = embed(&candidate_texts);
    let row = |vector: &[f32]| -> BTreeMap<String, f64> {
        golden
            .propositions
            .iter()
            .zip(&proposition_vectors)
            .map(|(proposition, other)| (proposition.id.clone(), cosine(vector, other)))
            .collect()
    };
    let similarities = Similarities {
        model: preset.repo_id().to_owned(),
        revision: preset.revision().to_owned(),
        input_kind: "symmetric".to_owned(),
        candidates: candidate_vectors.iter().map(|vector| row(vector)).collect(),
    };

    // Threshold calibration from golden data only: each expected quote against its own
    // proposition and the others, and the propositions against each other.
    let quoted: Vec<_> = golden
        .expected_attributions
        .iter()
        .filter(|expected| expected.quote.is_some())
        .collect();
    let quote_texts: Vec<&str> = quoted.iter().map(|e| e.quote.as_deref().unwrap()).collect();
    let quote_vectors = embed(&quote_texts);
    let calibration: Vec<Value> = quoted
        .iter()
        .zip(&quote_vectors)
        .map(|(expected, vector)| {
            let sims = row(vector);
            let best_other = sims
                .iter()
                .filter(|(id, _)| **id != expected.proposition)
                .map(|(_, value)| *value)
                .fold(f64::NEG_INFINITY, f64::max);
            json!({"proposition": expected.proposition, "quote": expected.quote,
                   "own": sims[&expected.proposition], "bestOther": best_other})
        })
        .collect();
    let proposition_pairs: Vec<Value> = golden
        .propositions
        .iter()
        .zip(&proposition_vectors)
        .enumerate()
        .flat_map(|(i, (a, va))| {
            golden.propositions[i + 1..]
                .iter()
                .zip(&proposition_vectors[i + 1..])
                .map(move |(b, vb)| json!({"from": a.id, "to": b.id, "cosine": cosine(va, vb)}))
                .collect::<Vec<_>>()
        })
        .collect();
    let embedding_seconds = started.elapsed().as_secs_f64();

    // Polarity (owner decision 2026-11-04): the pinned multilingual NLI preset judges every
    // claim/proposition pair at or above the lowest swept threshold. Premise: the claim's
    // canonical text; hypothesis: the golden proposition. A missing bundle fails the test.
    let started = Instant::now();
    let mut nli_options = NliPresetLoadOptions::default();
    if std::env::var_os("TEXT_CLASSIFICATION_MODEL_DIR").is_none() {
        nli_options.bundle_root =
            PathBuf::from(std::env::var("HOME").unwrap()).join(".cache/model-runtime");
    }
    let nli_preset = NliPreset::MdebertaV3BaseXnliMultilingual;
    let mut nli = load_nli_preset(nli_preset, &nli_options).expect("pinned NLI bundle");
    let polarity = Polarity {
        model: nli_preset.repo_id().to_owned(),
        revision: nli_preset.revision().to_owned(),
        candidates: candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                golden
                    .propositions
                    .iter()
                    .filter(|proposition| similarities.get(index, &proposition.id) >= SWEEP_MIN)
                    .map(|proposition| {
                        let judged = nli
                            .classify(&candidate.canonical_text, &proposition.text)
                            .unwrap();
                        let label = match judged.label {
                            text_classification::NliLabel::Entailment => NliLabel::Entailment,
                            text_classification::NliLabel::Neutral => NliLabel::Neutral,
                            text_classification::NliLabel::Contradiction => NliLabel::Contradiction,
                        };
                        let probabilities =
                            [judged.entailment, judged.neutral, judged.contradiction]
                                .map(f64::from);
                        (
                            proposition.id.clone(),
                            NliJudgment {
                                label,
                                probabilities,
                            },
                        )
                    })
                    .collect()
            })
            .collect(),
    };
    let nli_seconds = started.elapsed().as_secs_f64();
    let nli_pairs: usize = polarity.candidates.iter().map(BTreeMap::len).sum();

    let mapped = map_candidates(&document, &golden.people, &candidates);
    let score = score_with_polarity(
        &golden,
        &mapped,
        &similarities,
        Some(&polarity),
        ScoringConfig::with_threshold(SIMILARITY_THRESHOLD),
    )
    .unwrap();

    // Threshold sweep on this run, with and without the polarity check: how hits, fabrications
    // and the no-attribution rules move with the threshold. The fixed threshold above is chosen
    // from the polarity sweep (see the README).
    let summarize = |threshold: f64, at: &golden_scoring::stage4::AttributionScore| {
        json!({"threshold": threshold, "hits": at.hits, "partial": at.partial, "misses": at.misses,
               "fabricated": at.fabricated.len(),
               "noStanceEvidenceViolated": at.expected.iter().filter(|r| r.status == "violated").count(),
               "noAttributionHeld": at.no_attribution.iter().all(|rule| rule.held),
               "candidatesMatchingNoProposition": at.candidates_matching_no_proposition})
    };
    let steps = || (80..=95).map(|step| f64::from(step) / 100.0);
    let sweep: Vec<Value> = steps()
        .map(|threshold| {
            let at = score_with_polarity(
                &golden,
                &mapped,
                &similarities,
                Some(&polarity),
                ScoringConfig::with_threshold(threshold),
            )
            .unwrap();
            summarize(threshold, &at)
        })
        .collect();
    let sweep_cosine_only: Vec<Value> = steps()
        .map(|threshold| {
            summarize(
                threshold,
                &score_attributions(
                    &golden,
                    &mapped,
                    &similarities,
                    ScoringConfig::with_threshold(threshold),
                ),
            )
        })
        .collect();

    let write = |name: &str, value: &Value| {
        std::fs::write(out.join(name), serde_json::to_string_pretty(value).unwrap()).unwrap();
    };
    // Every candidate with the closest golden proposition, for review.
    let claims: Vec<Value> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let (best, similarity) = similarities.candidates[index]
                .iter()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(id, value)| (id.clone(), *value))
                .unwrap();
            let seconds = candidate
                .evidence
                .as_ref()
                .and_then(|evidence| document.seconds_at(evidence.start));
            json!({"candidate": index, "speaker": candidate.speaker, "seconds": seconds,
                   "canonicalText": candidate.canonical_text, "stanceCue": candidate.stance_cue,
                   "reporting": candidate.reporting.iter().map(|l| format!("{} [{}]", l.holder, l.cue)).collect::<Vec<_>>(),
                   "quote": candidate.evidence.as_ref().map(|e| e.quote.clone()),
                   "closestProposition": best, "similarity": similarity})
        })
        .collect();
    write(
        "similarities.json",
        &serde_json::to_value(&similarities).unwrap(),
    );
    write("polarity.json", &serde_json::to_value(&polarity).unwrap());
    write("claims.json", &json!(claims));
    let diagnostics = diagnose_stage4(
        &document,
        &candidates,
        &mapped,
        &similarities,
        Some(&polarity),
        &score,
    );
    write(
        "attributions-diagnostics.json",
        &serde_json::to_value(&diagnostics).unwrap(),
    );
    std::fs::write(
        out.join("attributions-diagnostics.md"),
        diagnostics_markdown(&diagnostics),
    )
    .unwrap();
    write(
        "attributions-report.json",
        &json!({
            "score": score,
            "calibration": {"quotes": calibration, "propositionPairs": proposition_pairs,
                            "thresholdSweep": sweep, "thresholdSweepCosineOnly": sweep_cosine_only},
            "polarity": {"model": polarity.model, "revision": polarity.revision, "pairs": nli_pairs,
                         "seconds": nli_seconds, "pairsFrom": SWEEP_MIN},
            "embeddingSeconds": embedding_seconds,
            "extraction": {"producer": extraction["producer"], "seconds": extraction["seconds"],
                           "windows": extraction["windows"], "diagnostics": extraction["diagnostics"]},
            "quoteOverlapMetric": "max share of either quote's words found in the other",
        }),
    );
    std::fs::write(
        out.join("attributions-summary.md"),
        summary_markdown(&score),
    )
    .unwrap();

    // The same attributions in belief-lab's own contracts, run through its attribution report.
    let provenance = ExportProvenance {
        exporter_revision: "git:belief-lab-golden-media".into(),
        source_repository: "belief-lab-golden".into(),
        scope_id: "russell-copleston-1948".into(),
        transcript_revision: format!("sha256:{}", sha256_hex(&document_bytes)),
        producer_name: "proposition-api".into(),
        producer_revision: format!("git:{}", extraction["producer"]["commit"].as_str().unwrap()),
        producer_model: extraction["producer"]["llamaModelFile"]
            .as_str()
            .unwrap_or("unknown")
            .into(),
    };
    let (batch, investigation) = belief_lab_export(
        &golden,
        &document,
        &mapped.attributions,
        &similarities,
        SIMILARITY_THRESHOLD,
        &provenance,
    );
    write("belief-lab-evidence.json", &batch);
    write("belief-lab-investigation.json", &investigation);
    let belief_report = compare(&ComparisonInput {
        evidence_json: &batch.to_string(),
        attributions_json: &investigation.to_string(),
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), "semantic_research".into())]),
    })
    .expect("belief-lab accepts the exported attributions");
    write(
        "belief-lab-report.json",
        &serde_json::to_value(belief_report).unwrap(),
    );

    let report_path = out.join("report.json");
    if let Ok(bytes) = std::fs::read(&report_path) {
        let mut report: Value = serde_json::from_slice(&bytes).unwrap();
        report["stages"]["attributions"] = json!({
            "status": "run",
            "threshold": SIMILARITY_THRESHOLD,
            "candidates": score.candidates,
            "verifiedCandidates": score.verified_candidates,
            "hits": score.hits, "partial": score.partial, "misses": score.misses,
            "fabricated": score.fabricated.len(),
            "noAttributionHeld": score.no_attribution.iter().all(|rule| rule.held),
            "report": "attributions-report.json",
        });
        write("report.json", &report);
    }
    println!("{}", summary_markdown(&score));

    assert_eq!(
        score.verified_candidates,
        candidates.iter().filter(|c| c.evidence.is_some()).count(),
        "every draft span must verify against the transcript and its speaker turn"
    );
    assert!(
        score.no_attribution.iter().all(|rule| rule.held),
        "expectedNoAttribution violated"
    );
    assert!(score.fabricated.is_empty(), "fabricated attributions");
}
