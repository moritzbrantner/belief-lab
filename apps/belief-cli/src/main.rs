use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use belief_core::{
    explain_claim, find_claim, infer_baseline, validate_bundle, BeliefEvidenceBundleV1,
    ExplanationNode,
};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "belief",
    version,
    about = "Inspect provenance-first belief evidence"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        input: PathBuf,
    },
    Infer {
        input: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Explain {
        input: PathBuf,
        claim_id: String,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { input } => {
            let validated = load(&input)?;
            println!(
                "valid: {} sources, {} spans, {} videos, {} evidence records",
                validated.source_count(),
                validated.span_count(),
                validated.video_count(),
                validated.evidence_count()
            );
        }
        Command::Infer { input, json } => {
            let validated = load(&input)?;
            let claims = infer_baseline(&validated);
            if json {
                println!("{}", serde_json::to_string_pretty(&claims)?);
            } else if claims.is_empty() {
                println!("no claims derived");
            } else {
                for claim in claims {
                    println!(
                        "{} = {} {} {} [support {:.2}, {:?}]",
                        claim.id,
                        claim.subject,
                        claim.predicate,
                        claim.object,
                        claim.support.value,
                        claim.support.semantics
                    );
                }
            }
        }
        Command::Explain {
            input,
            claim_id,
            json,
        } => {
            let validated = load(&input)?;
            let claims = infer_baseline(&validated);
            let claim = find_claim(&claims, &claim_id)?;
            let explanation = explain_claim(&validated, claim)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&explanation)?);
            } else {
                print_explanation(&explanation, 0);
            }
        }
    }
    Ok(())
}

fn load(path: &PathBuf) -> Result<belief_core::ValidatedBundle> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let bundle: BeliefEvidenceBundleV1 = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_bundle(bundle).map_err(Into::into)
}

fn print_explanation(node: &ExplanationNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let score = node
        .score
        .as_ref()
        .map(|score| format!(" · {:.2} {:?}", score.value, score.semantics))
        .unwrap_or_default();
    println!("{indent}- {} [{}]{}", node.label, node.kind, score);
    if let Some(detail) = &node.detail {
        println!("{indent}  {detail}");
    }
    if let Some(producer) = &node.producer {
        println!("{indent}  producer: {} @ {}", producer.name, producer.revision);
    }
    if let Some(group) = &node.correlation_group {
        println!("{indent}  correlation group: {group}");
    }
    for child in &node.children {
        print_explanation(child, depth + 1);
    }
}
