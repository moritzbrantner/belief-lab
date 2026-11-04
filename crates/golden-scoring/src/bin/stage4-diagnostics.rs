//! Offline replay of a persisted Stage-4 run, without ASR, embeddings or NLI inference.
use golden_scoring::stage4::{
    map_candidates, score_with_polarity, Candidate, Polarity, ScoringConfig, Similarities,
    StageFourDocument,
};
use golden_scoring::stage4_diagnostics::{diagnose_stage4, diagnostics_markdown};
use golden_scoring::GoldenExpectations;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&read(path)?)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid output filename: {}", path.display()))?;
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, contents)
        .map_err(|error| format!("{}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|error| format!("{}: {error}", path.display()))
}

fn replay(golden_path: &Path, run: &Path) -> Result<(), String> {
    let golden = GoldenExpectations::parse(
        &String::from_utf8(read(golden_path)?)
            .map_err(|error| format!("{}: {error}", golden_path.display()))?,
    )?;
    if !golden.is_approved() {
        return Err("the golden expectations have not been approved".into());
    }
    let document_path = run.join("stage4-document.json");
    let document_bytes = read(&document_path)?;
    let document: StageFourDocument = serde_json::from_slice(&document_bytes)
        .map_err(|error| format!("{}: {error}", document_path.display()))?;
    let extraction: Value = parse(&run.join("stage4-candidates.json"))?;
    if extraction["documentSha256"] != digest(&document_bytes) {
        return Err("candidate extraction was recorded for different document bytes".into());
    }
    let candidates: Vec<Candidate> = serde_json::from_value(extraction["candidates"].clone())
        .map_err(|error| format!("invalid saved candidates: {error}"))?;
    let similarities: Similarities = parse(&run.join("similarities.json"))?;
    let polarity: Polarity = parse(&run.join("polarity.json"))?;
    let previous: Value = parse(&run.join("attributions-report.json"))?;
    let config: ScoringConfig = serde_json::from_value(previous["score"]["config"].clone())
        .map_err(|error| format!("saved scoring configuration: {error}"))?;
    if similarities.candidates.len() != candidates.len()
        || polarity.candidates.len() != candidates.len()
    {
        return Err("saved similarities or polarity table has a different candidate count".into());
    }
    if candidates.iter().enumerate().any(|(index, _)| {
        golden
            .propositions
            .iter()
            .any(|proposition| !similarities.candidates[index].contains_key(&proposition.id))
    }) {
        return Err("saved similarity table is missing a candidate/proposition pair".into());
    }
    let mapped = map_candidates(&document, &golden.people, &candidates);
    let score = score_with_polarity(
        &golden,
        &mapped,
        &similarities,
        Some(&polarity),
        config,
    )?;
    let score_value = serde_json::to_value(&score).map_err(|error| error.to_string())?;
    if score_value != previous["score"] {
        return Err("replayed score differs from the saved baseline; inputs or scorer changed".into());
    }
    let diagnostic = diagnose_stage4(
        &document,
        &candidates,
        &mapped,
        &similarities,
        Some(&polarity),
        &score,
    );
    let data = serde_json::to_vec_pretty(&diagnostic).map_err(|error| error.to_string())?;
    write_atomic(&run.join("attributions-diagnostics.json"), &data)?;
    write_atomic(
        &run.join("attributions-diagnostics.md"),
        diagnostics_markdown(&diagnostic).as_bytes(),
    )?;
    println!(
        "Replayed {} expected entries, {} fabricated attributions and {} negative rules without models.\nDiagnostics: {}",
        diagnostic.expected.len(),
        diagnostic.fabricated.len(),
        diagnostic.no_attribution.len(),
        run.join("attributions-diagnostics.json").display()
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!(
            "Usage: stage4-diagnostics <expected-beliefs.json> <saved-stage4-run-directory>"
        );
        std::process::exit(2);
    }
    if let Err(error) = replay(&PathBuf::from(&args[1]), &PathBuf::from(&args[2])) {
        eprintln!("Stage-4 diagnostic replay failed: {error}");
        std::process::exit(1);
    }
}
