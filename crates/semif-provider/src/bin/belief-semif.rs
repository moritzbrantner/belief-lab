use std::env;
use std::path::Path;
use std::process::ExitCode;

use semif_provider::{
    bootstrap_semif, download_model, model_catalog, SemifInstallBackend, SemifModelTier,
};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("belief-semif: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.as_slice() {
        [command] if command == "catalog" => {
            print_catalog();
            Ok(())
        }
        [command, directory] if command == "bootstrap" => {
            bootstrap(Path::new(directory), SemifInstallBackend::LlamaCpp)
        }
        [command, directory, backend] if command == "bootstrap" => {
            let backend = SemifInstallBackend::parse(backend).ok_or_else(|| {
                format!("unknown backend {backend:?}; use torch, mlx, or llamacpp")
            })?;
            bootstrap(Path::new(directory), backend)
        }
        [command, tier, directory] if command == "download-model" => {
            let tier = SemifModelTier::parse(tier).ok_or_else(|| {
                format!("unknown model tier {tier:?}; use phone, desktop, or high-memory")
            })?;
            let path =
                download_model(tier, Path::new(directory)).map_err(|error| error.to_string())?;
            println!("Model ready at {}.", path.display());
            Ok(())
        }
        _ => Err(usage().into()),
    }
}

fn print_catalog() {
    println!("tier\tmodel\tmodel_revision\tgguf_repository\tgguf_revision\tbytes");
    for pin in model_catalog() {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            pin.tier.as_str(),
            pin.source,
            pin.source_revision,
            pin.gguf_repository,
            pin.gguf_revision,
            pin.gguf_bytes
        );
    }
}

fn bootstrap(directory: &Path, backend: SemifInstallBackend) -> Result<(), String> {
    let outcome = bootstrap_semif(directory, backend).map_err(|error| error.to_string())?;
    println!(
        "SemIf is ready in {} using {} (cloned: {}, venv created: {}).",
        directory.display(),
        backend.as_str(),
        outcome.cloned,
        outcome.venv_created
    );
    Ok(())
}

fn usage() -> &'static str {
    "usage:\n  belief-semif catalog\n  belief-semif bootstrap <directory> [torch|mlx|llamacpp]\n  belief-semif download-model <phone|desktop|high-memory> <directory>"
}
