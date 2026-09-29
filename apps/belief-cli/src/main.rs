mod workbench;

use belief_cli::explain;

use belief_cli::pipeline;
use belief_cli::render;

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use belief_core::{
    BeliefId, Claim, ClaimId, EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId,
    EvidencePurpose, EvidenceRef, InferenceRunId, JudgmentId, JudgmentOutcome, JudgmentSpecRef,
    ObjectValue, Predicate, ProducerRef, Proposition, Provenance, SourceRef,
};
use belief_policy::{PolicyConfig, POLICY_KEYS};
use belief_store::InMemoryBeliefStore;
use inference_core::{EvidenceUse, InferenceRequest, JudgmentBasis, TrustedInferenceRule};
use semantic_decision::{
    DecisionEvidenceUse, DecisionOption, DecisionRequest, SemanticDecisionEngine,
    CONTRADICTS_OPTION_ID, SUPPORTS_OPTION_ID, UNKNOWN_OPTION_ID,
};
use semif_provider::{
    bootstrap_semif, download_model, model_is_ready, model_path, semif_install_is_ready,
    semif_score_path, SemifBackend, SemifInstallBackend, SemifModelTier, SemifProvider,
};

use explain::{explain, ExplainInput, InferenceStatus, Outcome};
use pipeline::derive_belief;

const SEMIF_DIR: &str = ".local/semif";
const MODEL_DIR: &str = ".local/models";

/// The offline demo is an ordinary explain fixture, so `cargo run` and `explain` share one path.
const DEMO_EVIDENCE: &str = include_str!("../../../fixtures/explain/offline-demo/evidence.json");
const DEMO_JUDGMENTS: &str = include_str!("../../../fixtures/explain/offline-demo/judgments.json");
const DEMO_PROFILE: &str = "semantic_research";

/// `explain` exit code when an input is malformed or unsupported and the run failed closed.
const EXIT_INPUT_REJECTED: u8 = 2;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("belief: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, Box<dyn Error>> {
    if args.first().map(String::as_str) == Some("decide") {
        return Ok(run_decide(&args[1..]));
    }
    if args.first().map(String::as_str) == Some("explain") {
        return run_explain(&args[1..]);
    }
    run_command(&args)?;
    Ok(ExitCode::SUCCESS)
}

fn run_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args {
        [] => {
            print_core_demo(run_core_demo()?);
            Ok(())
        }
        [command] if command == "demo" => {
            print_core_demo(run_core_demo()?);
            Ok(())
        }
        [command] if command == "workbench" => workbench::run(execute_local),
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
    proposition: String,
    belief_value: f64,
    source_repository: String,
    producer: String,
}

fn run_core_demo() -> Result<DemoSummary, Box<dyn Error>> {
    let report = explain(&ExplainInput {
        evidence_json: DEMO_EVIDENCE,
        judgments_json: Some(DEMO_JUDGMENTS),
        policy: BTreeMap::from([("BELIEF_POLICY_PROFILE".into(), DEMO_PROFILE.into())]),
    })?;
    let inference = report
        .inferences
        .first()
        .filter(|inference| inference.status == InferenceStatus::Derived)
        .ok_or_else(|| app_error("demo did not derive a belief"))?;
    let belief = inference
        .belief
        .as_ref()
        .ok_or_else(|| app_error("demo belief is missing"))?;
    let retained = inference
        .provenance
        .as_ref()
        .and_then(|provenance| provenance.evidence.first())
        .ok_or_else(|| app_error("demo explanation did not retain evidence"))?;
    let proposition = &inference.proposition;

    Ok(DemoSummary {
        proposition: format!(
            "{} {} {}",
            proposition.subject, proposition.predicate, proposition.object
        ),
        belief_value: belief.value,
        source_repository: retained.source.repository.clone(),
        producer: retained.producer.name.clone(),
    })
}

fn print_core_demo(summary: DemoSummary) {
    println!("Belief Lab is ready.");
    println!(
        "Derived {} as {:.3} soft truth.",
        summary.proposition, summary.belief_value
    );
    println!(
        "Explanation retained source {} and producer {}.",
        summary.source_repository, summary.producer
    );
    println!("No model download or .env file was required.");
    println!();
    println!("To explain any evidence batch offline:");
    println!("  cargo run -q -- explain fixtures/explain/offline-demo/evidence.json \\");
    println!(
        "    --judgments fixtures/explain/offline-demo/judgments.json --profile {DEMO_PROFILE}"
    );
    println!();
    println!("For real local semantic scoring:");
    println!("  cargo run -- setup");
    println!("  cargo run -- semantic-demo");
}

fn setup(tier: SemifModelTier) -> Result<(), Box<dyn Error>> {
    let paths = LocalPaths::default();
    eprintln!(
        "Preparing SemIf {} and the {} model tier...",
        semif_provider::SEMIF_SOURCE_REVISION,
        tier.as_str()
    );
    let outcome = bootstrap_semif(&paths.semif, SemifInstallBackend::LlamaCpp)?;
    let model = download_model(tier, &paths.models)?;
    semif_provider::prepare_tokenizer(tier, &paths.semif)?;
    eprintln!(
        "SemIf ready at {} (cloned: {}, venv created: {}).",
        outcome.executable.display(),
        outcome.cloned,
        outcome.venv_created
    );
    eprintln!("Model ready at {}.", model.display());
    eprintln!("Run cargo run -- semantic-demo {}.", tier.as_str());
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
    let model_ready = model_is_ready(tier, &paths.models);
    match &model_ready {
        Ok(true) => println!(
            "  model {}: ready ({})",
            tier.as_str(),
            model_path(tier, &paths.models).display()
        ),
        Ok(false) => println!("  model {}: not downloaded", tier.as_str()),
        Err(error) => println!("  model {}: invalid ({error})", tier.as_str()),
    }

    if !semif_ready || !model_ready.unwrap_or(false) {
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
        judgment,
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
        claim,
        vec![basis],
    )?;

    let mut store = InMemoryBeliefStore::default();
    store.insert_evidence(evidence)?;
    let derived = derive_belief(&mut store, &policy, inference, |store| {
        store.insert_semantic_judgment(semantic)
    })?;
    println!(
        "Belief Lab produced {:.3} soft truth with provider provenance retained.",
        derived.explanation.belief.value.value().value()
    );
    Ok(())
}

/// `belief explain <evidence.json> [--judgments <file>] [--profile <name>]
/// [--policy KEY=VALUE]... [--json]`
fn run_explain(args: &[String]) -> Result<ExitCode, Box<dyn Error>> {
    let options = ExplainOptions::parse(args)?;
    let evidence_json = read_input(&options.evidence)?;
    let judgments_json = options.judgments.as_deref().map(read_input).transpose()?;

    let report = explain(&ExplainInput {
        evidence_json: &evidence_json,
        judgments_json: judgments_json.as_deref(),
        policy: options.policy,
    })?;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render::render_text(&report));
    }

    Ok(match report.outcome {
        Outcome::Explained => ExitCode::SUCCESS,
        Outcome::InputRejected => ExitCode::from(EXIT_INPUT_REJECTED),
    })
}

#[derive(Debug, PartialEq)]
struct ExplainOptions {
    evidence: PathBuf,
    judgments: Option<PathBuf>,
    policy: BTreeMap<String, String>,
    json: bool,
}

impl ExplainOptions {
    fn parse(args: &[String]) -> Result<Self, Box<dyn Error>> {
        let mut evidence = None;
        let mut judgments = None;
        let mut policy = BTreeMap::new();
        let mut json = false;
        let mut args = args.iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--json" => json = true,
                "--judgments" => {
                    let value = args.next().ok_or("--judgments requires a file path")?;
                    if judgments.replace(PathBuf::from(value)).is_some() {
                        return Err(app_error("--judgments may be given once"));
                    }
                }
                "--profile" => {
                    let value = args.next().ok_or("--profile requires a profile name")?;
                    insert_policy(&mut policy, "BELIEF_POLICY_PROFILE", value)?;
                }
                "--policy" => {
                    let value = args.next().ok_or("--policy requires KEY=VALUE")?;
                    let (key, value) = value.split_once('=').ok_or_else(|| {
                        app_error(format!("--policy expects KEY=VALUE, got {value:?}"))
                    })?;
                    if key == "BELIEF_POLICY_PROFILE" {
                        return Err(app_error("use --profile to select the policy profile"));
                    }
                    if !POLICY_KEYS.contains(&key) {
                        return Err(app_error(format!(
                            "unknown policy key {key:?}; expected one of {}",
                            POLICY_KEYS.join(", ")
                        )));
                    }
                    insert_policy(&mut policy, key, value)?;
                }
                flag if flag.starts_with("--") => {
                    return Err(app_error(format!("unknown explain option {flag}")));
                }
                path => {
                    if evidence.replace(PathBuf::from(path)).is_some() {
                        return Err(app_error("explain takes exactly one evidence file"));
                    }
                }
            }
        }

        Ok(Self {
            evidence: evidence.ok_or("explain requires an evidence interchange JSON file")?,
            judgments,
            policy,
            json,
        })
    }
}

fn insert_policy(
    policy: &mut BTreeMap<String, String>,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn Error>> {
    if policy.insert(key.into(), value.into()).is_some() {
        return Err(app_error(format!("{key} may be set once")));
    }
    Ok(())
}

fn read_input(path: &Path) -> Result<String, Box<dyn Error>> {
    fs::read_to_string(path)
        .map_err(|error| app_error(format!("cannot read {}: {error}", path.display())))
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
        "Belief Lab

Usage:
  cargo run                         Run the offline core demo
  cargo run -- demo                 Run the offline core demo
  cargo run -- explain <evidence.json> [--judgments <file>] [--profile <name>]
                       [--policy KEY=VALUE]... [--json]
                                    Explain an evidence batch offline (exit 2 = input rejected)
  cargo run -- workbench            Open a local browser workbench for real models
  cargo run -- decide <request.json|-> Execute one authorized semantic decision as JSON
  cargo run -- setup [tier]         Install/update pinned SemIf and download a model
  cargo run -- semantic-demo [tier] Run a real local SemIf-backed decision
  cargo run -- doctor [tier]        Show local prerequisites and setup state
  cargo run -- help                 Show this help

Profiles: observe_only (default), semantic_research, multimodal_research
Model tiers: phone (default), desktop, high-memory"
    );
}

fn run_decide(args: &[String]) -> ExitCode {
    use belief_cli::decision::{Failure, MAX_INPUT_BYTES};
    let run = || -> Result<serde_json::Value, Failure> {
        let [path] = args else {
            return Err(Failure::new(
                "usage",
                "decide requires a request.json file or - for stdin",
            ));
        };
        let reader: Box<dyn Read> = if path == "-" {
            Box::new(io::stdin())
        } else {
            Box::new(fs::File::open(path).map_err(|e| Failure::new("input_io", e))?)
        };
        let mut input = String::new();
        reader
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_string(&mut input)
            .map_err(|e| Failure::new("input_io", e))?;
        execute_local(&input)
    };
    match run() {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", error.report());
            ExitCode::from(2)
        }
    }
}

fn execute_local(input: &str) -> Result<serde_json::Value, belief_cli::decision::Failure> {
    use belief_cli::decision::{execute, Failure, FixtureEngine, ProviderSelection};
    execute(input, |selection| match selection {
        ProviderSelection::Fixture { scores } => Ok(Box::new(FixtureEngine {
            scores: scores.clone(),
        })),
        ProviderSelection::Semif { tier } => {
            let tier = parse_tier(tier).map_err(|e| Failure::new("invalid_provider", e))?;
            let paths = LocalPaths::default();
            setup(tier).map_err(|e| Failure::new("setup_failed", e))?;
            Ok(Box::new(SemifProvider::from_bootstrap(
                &paths.semif,
                tier.pin(),
                SemifBackend::LlamaCpp {
                    gguf: model_path(tier, &paths.models),
                    threads: None,
                },
            )))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_demo_exercises_the_full_belief_pipeline() {
        let summary = run_core_demo().unwrap();

        assert_eq!(
            summary.proposition,
            "person:alice prefers_customization true"
        );
        assert!((summary.belief_value - 0.95).abs() < f64::EPSILON);
        assert_eq!(summary.source_repository, "belief-cli-demo");
        assert_eq!(summary.producer, "belief-cli-demo");
    }

    fn explain_args(args: &[&str]) -> Result<ExplainOptions, Box<dyn Error>> {
        ExplainOptions::parse(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn explain_options_map_flags_to_policy_pairs() {
        let options = explain_args(&[
            "batch.json",
            "--profile",
            "multimodal_research",
            "--policy",
            "BELIEF_BIOMETRIC_EVIDENCE=reference_only",
            "--json",
        ])
        .unwrap();

        assert_eq!(options.evidence, Path::new("batch.json"));
        assert_eq!(options.judgments, None);
        assert!(options.json);
        assert_eq!(
            options.policy,
            BTreeMap::from([
                (
                    "BELIEF_BIOMETRIC_EVIDENCE".to_string(),
                    "reference_only".to_string()
                ),
                (
                    "BELIEF_POLICY_PROFILE".to_string(),
                    "multimodal_research".to_string()
                ),
            ])
        );
    }

    #[test]
    fn explain_options_reject_ambiguous_or_unknown_settings() {
        assert!(explain_args(&[]).is_err());
        assert!(explain_args(&["a.json", "b.json"]).is_err());
        assert!(
            explain_args(&["a.json", "--policy", "BELIEF_POLICY_PROFILE=observe_only"]).is_err()
        );
        assert!(explain_args(&["a.json", "--policy", "BELIEF_UNKNOWN=true"]).is_err());
        assert!(explain_args(&["a.json", "--policy", "BELIEF_BIOMETRIC_EVIDENCE"]).is_err());
        assert!(explain_args(&[
            "a.json",
            "--profile",
            "observe_only",
            "--profile",
            "semantic_research"
        ])
        .is_err());
        assert!(explain_args(&["a.json", "--verbose"]).is_err());
    }

    #[test]
    fn local_paths_are_repository_relative() {
        let paths = LocalPaths::default();

        assert_eq!(paths.semif, Path::new(".local/semif"));
        assert_eq!(paths.models, Path::new(".local/models"));
    }
}
