use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use semif_provider::{model_catalog, SemifModelTier, SEMIF_REPOSITORY, SEMIF_SOURCE_REVISION};

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
            bootstrap(Path::new(directory), "llamacpp")
        }
        [command, directory, backend] if command == "bootstrap" => {
            bootstrap(Path::new(directory), backend)
        }
        [command, tier, directory] if command == "download-model" => {
            let tier = SemifModelTier::parse(tier).ok_or_else(|| {
                format!("unknown model tier {tier:?}; use phone, desktop, or high-memory")
            })?;
            download_model(tier, Path::new(directory))
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

fn bootstrap(directory: &Path, backend: &str) -> Result<(), String> {
    let extra = match backend {
        "torch" => ".",
        "mlx" => ".[mlx]",
        "llamacpp" => ".[llamacpp]",
        _ => {
            return Err(format!(
                "unknown backend {backend:?}; use torch, mlx, or llamacpp"
            ))
        }
    };

    if directory.exists() {
        return Err(format!(
            "destination {} already exists; bootstrap is create-only",
            directory.display()
        ));
    }

    run_command(
        Command::new("git")
            .arg("clone")
            .arg("--no-checkout")
            .arg(SEMIF_REPOSITORY)
            .arg(directory),
        "clone SemIf",
    )?;
    run_command(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("checkout")
            .arg("--detach")
            .arg(SEMIF_SOURCE_REVISION),
        "check out pinned SemIf revision",
    )?;

    let python = env::var("BELIEF_SEMIF_PYTHON").unwrap_or_else(|_| "python3".into());
    run_command(
        Command::new(&python)
            .arg("-m")
            .arg("venv")
            .arg(directory.join(".venv")),
        "create SemIf virtual environment",
    )?;

    let interpreter = fs::canonicalize(venv_python(directory)).map_err(|error| {
        format!(
            "could not resolve SemIf virtual-environment interpreter: {error}"
        )
    })?;

    run_command(
        Command::new(interpreter)
            .current_dir(directory)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("-e")
            .arg(extra),
        "install pinned SemIf",
    )?;

    println!(
        "Installed SemIf {} in {} with backend {}.",
        SEMIF_SOURCE_REVISION,
        directory.display(),
        backend
    );
    Ok(())
}

fn download_model(tier: SemifModelTier, directory: &Path) -> Result<(), String> {
    let pin = tier.pin();
    fs::create_dir_all(directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;

    let target = directory.join(pin.gguf_file);
    if target.exists() {
        verify_size(&target, pin.gguf_bytes)?;
        println!(
            "{} is already present and has the expected size.",
            target.display()
        );
        return Ok(());
    }

    let partial = directory.join(format!("{}.part", pin.gguf_file));
    run_command(
        Command::new("curl")
            .arg("--fail")
            .arg("--location")
            .arg("--continue-at")
            .arg("-")
            .arg("--output")
            .arg(&partial)
            .arg(pin.gguf_url()),
        "download pinned GGUF",
    )?;

    verify_size(&partial, pin.gguf_bytes)?;
    fs::rename(&partial, &target).map_err(|error| {
        format!(
            "could not move {} to {}: {error}",
            partial.display(),
            target.display()
        )
    })?;

    println!(
        "Downloaded {} ({}) at immutable revision {} to {}.",
        pin.source,
        pin.tier.as_str(),
        pin.gguf_revision,
        target.display()
    );
    Ok(())
}

fn verify_size(path: &Path, expected: u64) -> Result<(), String> {
    let actual = fs::metadata(path)
        .map_err(|error| format!("could not stat {}: {error}", path.display()))?
        .len();

    if actual != expected {
        return Err(format!(
            "{} has {actual} bytes; expected {expected}. Remove the file and retry.",
            path.display()
        ));
    }

    Ok(())
}

fn run_command(command: &mut Command, purpose: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("could not {purpose}: {error}"))?;
    if !status.success() {
        return Err(format!("could not {purpose}: process exited with {status}"));
    }
    Ok(())
}

fn venv_python(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join(".venv").join("Scripts").join("python.exe")
    } else {
        root.join(".venv").join("bin").join("python")
    }
}

fn usage() -> &'static str {
    "usage:\n  belief-semif catalog\n  belief-semif bootstrap <directory> [torch|mlx|llamacpp]\n  belief-semif download-model <phone|desktop|high-memory> <directory>"
}
