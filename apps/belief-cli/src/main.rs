use std::env;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use belief_core::{
    BeliefId, Claim, ClaimId, EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId,
    EvidencePurpose, EvidenceRef, InferenceRunId, Judgment, JudgmentId, JudgmentOutcome,
    JudgmentSpecRef, ObjectValue, Predicate, ProducerRef, Proposition, Provenance, Score,
    ScoreSemantics, SourceRef,
};
use belief_policy::PolicyConfig;
use belief_store::InMemoryBeliefStore;
use inference_baseline::BaselineInferenceEngine;
use inference_core::{
    EvidenceUse, InferenceEngine, InferenceRequest, JudgmentBasis, TrustedInferenceRule,
};
use semantic_decision::{
    DecisionEvidenceUse, DecisionOption, DecisionRequest, SemanticDecisionEngine,
    CONTRADICTS_OPTION_ID, SUPPORTS_OPTION_ID, UNKNOWN_OPTION_ID,
};
use semif_provider::{
    bootstrap_semif, download_model, model_is_ready, model_path, semif_install_is_ready,
    semif_score_path, SemifBackend, SemifInstallBackend, SemifModelTier, SemifProvider,
};

const SEMIF_DIR: &str = ".local/semif";
const MODEL_DIR: &str = ".local/models";

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("belief: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), Box<dyn Error>> {
    match args.as_slice() {
        [] => {
            print_core_demo(run_core_demo()?);
            Ok(())
        }
        [command] if command == "demo" => {
            print_core_demo(run_core_demo()?);
            Ok(())
        }
        [command] if command == "setup" => setup(SemifModelTier::Phone),
        [command, tier] if command == "setup" => setup(parse_tier(tier)?),
        [command] if command == "doctor" => doctor(SemifModelTier::Phone),
        [command, tier] if command == "doctor" => doctor(parse_tier(tier)?),
        [command] if command == "semantic-demo" => semantic_demo(SemifModelTier::Phone),
        [command, tier] if command == "semantic-demo" => semantic_demo(parse_tier(tier)?),
        [command] if command == "help" || command == "--help" || command == "-h" => {
            print_usage();
            Ok(())
        }
        _ => Err(app_error("unknown command; run cargo run -- help")),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DemoSummary {
    belief_value: f64,
    source_repository: String,
    producer: String,
}

fn run_core_demo() -> Result<DemoSummary, Box<dyn Error>> {
    let proposition = Proposition::new(
        EntityId::new("person:alice")?,
        Predicate::new("prefers_customization")?,
        ObjectValue::Boolean(true),
    );
    let evidence = EvidenceRef::new(
        EvidenceId::new("evidence:demo:statement")?,
        Some(EntityId::new("person:alice")?),
        EvidenceClass::Transcript,
        EvidenceFamilyId::new("family:demo:statement")?,
        None,
        Provenance::new(
            SourceRef::new(
                "belief-cli-demo",
                "demo:1",
                "statement:1",
                "sha256:demo-source-v1",
            )?,
            ProducerRef::new("belief-cli-demo", "commit:demo-producer-v1", None, None)?,
            [],
        ),
    );
    let judgment = Judgment::new(
        JudgmentId::new("judgment:demo:support")?,
        proposition.clone(),
        JudgmentOutcome::Supports,
        Score::new(0.9, ScoreSemantics::ModelConfidence)?,
        [evidence.id.clone()],
        JudgmentSpecRef::new("demo-support", "v1")?,
        "demo-model:v1",
    )?;
    let claim = Claim::from_judgment(
        ClaimId::new("claim:demo:preference")?,
        proposition,
        judgment.id().clone(),
    );
    let basis = JudgmentBasis::new(
        judgment.clone(),
        EvidenceFamilyId::new("correlation:demo:statement")?,
        vec![EvidenceUse::new(
            evidence.clone(),
            EvidencePurpose::Corroboration,
        )],
    )?;
    let request = InferenceRequest::new(
        InferenceRunId::new("run:demo:1")?,
        BeliefId::new("belief:demo:1")?,
        TrustedInferenceRule::BaselinePreferenceV1,
        claim.clone(),
        vec![basis],
    )?;
    let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")])?;
    let authorized = request.authorize(&policy)?;
    let result = BaselineInferenceEngine.infer(&authorized)?;
    let belief_id = result.belief().id().clone();

    let mut store = InMemoryBeliefStore::default();
    store.insert_evidence(evidence)?;
    store.insert_judgment(judgment)?;
    store.insert_claim(claim)?;
    store.insert_inference_result(result)?;

    let explanation = store.explain_belief(&belief_id)?;
    let retained = explanation
        .evidence
        .first()
        .ok_or_else(|| app_error("demo explanation did not retain evidence"))?;

    Ok(DemoSummary {
        belief_value: explanation.belief.value.value().value(),
        source_repository: retained.value.provenance.source.repository().to_string(),
        producer: retained.value.provenance.producer.name().to_string(),
    })
}

fn print_core_demo(summary: DemoSummary) {
    println!("Belief Lab is ready.");
    println!(
        "Derived person:alice prefers_customization true as {:.3} soft truth.",
        summary.belief_value
    );
    println!(
        "Explanation retained source {} and producer {}.",
        summary.source_repository, summary.producer
    );
    println!("No model download or .env file was required.");
    println!();
    println!("For real local semantic scoring:");
    println!("  cargo run -- setup");
    println!("  cargo run -- semantic-demo");
}

fn setup(tier: SemifModelTier) -> Result<(), Box<dyn Error>> {
    let paths = LocalPaths::default();
    println!(
        "Preparing SemIf {} and the {} model tier...",
        semif_provider::SEMIF_SOURCE_REVISION,
        tier.as_str()
    );
    let outcome = bootstrap_semif(&paths.semif, SemifInstallBackend::LlamaCpp)?;
    let model = download_model(tier, &paths.models)?;
    println!(
        "SemIf ready at {} (cloned: {}, venv created: {}).",
        outcome.executable.display(),
        outcome.cloned,
        outcome.venv_created
    );
    println!("Model ready at {}.", model.display());
    println!("Run cargo run -- semantic-demo {}.", tier.as_str());
    Ok(())
}

fn doctor(tier: SemifModelTier) -> Result<(), Box<dyn Error>> {
    let paths = LocalPaths::default();
    let python = env::var("BELIEF_SEMIF_PYTHON").unwrap_or_else(|_| "python3".into());
    println!("Belief Lab doctor");
    println!("  git: {}", availability("git"));
    println!("  {python}: {}", availability(&python));
    println!("  curl: {}", availability("curl"));

    let executable = semif_score_path(&paths.semif);
    let semif_ready = semif_install_is_ready(&paths.semif, SemifInstallBackend::LlamaCpp);
    println!(
        "  SemIf: {} ({})",
        if semif_ready { "ready" } else { "not ready" },
        executable.display()
    );
    match model_is_ready(tier, &paths.models) {
        Ok(true) => println!(
            "  model {}: ready ({})",
            tier.as_str(),
            model_path(tier, &paths.models).display()
        ),
        Ok(false) => println!("  model {}: not downloaded", tier.as_str()),
        Err(error) => println!("  model {}: invalid ({error})", tier.as_str()),
    }

    if !semif_ready || !model_is_ready(tier, &paths.models).unwrap_or(false) {
        println!();
        println!(
            "Run cargo run -- setup {} to prepare local semantic scoring.",
            tier.as_str()
        );
    }
    Ok(())
}

fn semantic_demo(tier: SemifModelTier) -> Result<(), Box<dyn Error>> {
    let paths = LocalPaths::default();
    if !semif_install_is_ready(&paths.semif, SemifInstallBackend::LlamaCpp)
        || !model_is_ready(tier, &paths.models)?
    {
        return Err(app_error(format!(
            "local SemIf is not ready; run cargo run -- setup {} first",
            tier.as_str()
        )));
    }

    let proposition = Proposition::new(
        EntityId::new("person:alice")?,
        Predicate::new("prefers_customization")?,
        ObjectValue::Boolean(true),
    );
    let evidence = EvidenceRef::new(
        EvidenceId::new("evidence:semantic-demo:statement")?,
        Some(EntityId::new("person:alice")?),
        EvidenceClass::Transcript,
        EvidenceFamilyId::new("family:semantic-demo:statement")?,
        None,
        Provenance::new(
            SourceRef::new(
                "belief-cli-demo",
                "semantic-demo:1",
                "statement:1",
                "sha256:semantic-demo-source-v1",
            )?,
            ProducerRef::new(
                "belief-cli-demo",
                "commit:semantic-demo-producer-v1",
                None,
                None,
            )?,
            [],
        ),
    );

    let decision = DecisionRequest::new(
        "decision:semantic-demo:1",
        serde_json::Value::String(
            "Alice explicitly says: I prefer software that lets me customize how it works.".into(),
        ),
        "Does this evidence support the proposition that Alice prefers customization?",
        vec![
            DecisionOption::new(SUPPORTS_OPTION_ID, "The evidence supports the proposition.")?,
            DecisionOption::new(
                CONTRADICTS_OPTION_ID,
                "The evidence contradicts the proposition.",
            )?,
            DecisionOption::new(UNKNOWN_OPTION_ID, "The evidence is insufficient to decide.")?,
        ],
        belief_core::InferenceClass::Preference,
        vec![DecisionEvidenceUse::new(
            evidence.clone(),
            EvidencePurpose::Corroboration,
        )],
    )?;
    let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")])?;
    let authorized_decision = decision.authorize(&policy)?;
    let pin = tier.pin();
    let provider = SemifProvider::from_bootstrap(
        &paths.semif,
        pin,
        SemifBackend::LlamaCpp {
            gguf: model_path(tier, &paths.models),
            threads: None,
        },
    );
    let receipt = provider.decide(&authorized_decision)?;
    let semantic = authorized_decision.into_three_way_judgment(
        receipt,
        JudgmentId::new("judgment:semantic-demo:1")?,
        proposition.clone(),
        JudgmentSpecRef::new("semantic-demo-support", "v1")?,
    )?;

    println!(
        "SemIf selected {} with {:.3} conditional option probability.",
        semantic.provenance().decision().selected_option(),
        semantic.judgment().confidence().value()
    );
    println!(
        "Model: {} @ {}",
        semantic.provenance().decision().model(),
        semantic.provenance().decision().model_revision()
    );

    if semantic.judgment().outcome() == JudgmentOutcome::Unknown {
        println!("The decision was unknown, so Belief Lab correctly produced no belief.");
        return Ok(());
    }

    let judgment = semantic.judgment().clone();
    let claim = Claim::from_judgment(
        ClaimId::new("claim:semantic-demo:1")?,
        proposition,
        judgment.id().clone(),
    );
    let basis = JudgmentBasis::new(
        judgment.clone(),
        EvidenceFamilyId::new("correlation:semantic-demo:statement")?,
        vec![EvidenceUse::new(
            evidence.clone(),
            EvidencePurpose::Corroboration,
        )],
    )?;
    let inference = InferenceRequest::new(
        InferenceRunId::new("run:semantic-demo:1")?,
        BeliefId::new("belief:semantic-demo:1")?,
        TrustedInferenceRule::BaselinePreferenceV1,
        claim.clone(),
        vec![basis],
    )?
    .authorize(&policy)?;
    let result = BaselineInferenceEngine.infer(&inference)?;
    let belief_id = result.belief().id().clone();

    let mut store = InMemoryBeliefStore::default();
    store.insert_evidence(evidence)?;
    store.insert_semantic_judgment(semantic)?;
    store.insert_claim(claim)?;
    store.insert_inference_result(result)?;

    let explanation = store.explain_belief(&belief_id)?;
    println!(
        "Belief Lab produced {:.3} soft truth with provider provenance retained.",
        explanation.belief.value.value().value()
    );
    Ok(())
}

fn parse_tier(value: &str) -> Result<SemifModelTier, Box<dyn Error>> {
    SemifModelTier::parse(value).ok_or_else(|| {
        app_error(format!(
            "unknown model tier {value:?}; use phone, desktop, or high-memory"
        ))
    })
}

fn availability(command: &str) -> &'static str {
    match Command::new(command).arg("--version").output() {
        Ok(output) if output.status.success() => "available",
        _ => "missing",
    }
}

#[derive(Debug)]
struct LocalPaths {
    semif: PathBuf,
    models: PathBuf,
}

impl Default for LocalPaths {
    fn default() -> Self {
        Self {
            semif: Path::new(SEMIF_DIR).to_path_buf(),
            models: Path::new(MODEL_DIR).to_path_buf(),
        }
    }
}

fn app_error(message: impl Into<String>) -> Box<dyn Error> {
    io::Error::other(message.into()).into()
}

fn print_usage() {
    println!(
        "Belief Lab\\n\\n\\
         Usage:\\n\\
           cargo run                         Run the offline core demo\\n\\
           cargo run -- demo                 Run the offline core demo\\n\\
           cargo run -- setup [tier]         Install/update pinned SemIf and download a model\\n\\
           cargo run -- semantic-demo [tier] Run a real local SemIf-backed decision\\n\\
           cargo run -- doctor [tier]        Show local prerequisites and setup state\\n\\
           cargo run -- help                 Show this help\\n\\n\\
         Model tiers: phone (default), desktop, high-memory"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_demo_exercises_the_full_belief_pipeline() {
        let summary = run_core_demo().unwrap();

        assert!((summary.belief_value - 0.95).abs() < f64::EPSILON);
        assert_eq!(summary.source_repository, "belief-cli-demo");
        assert_eq!(summary.producer, "belief-cli-demo");
    }

    #[test]
    fn local_paths_are_repository_relative() {
        let paths = LocalPaths::default();

        assert_eq!(paths.semif, Path::new(".local/semif"));
        assert_eq!(paths.models, Path::new(".local/models"));
    }
}
