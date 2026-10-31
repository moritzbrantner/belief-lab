use crate::error::SemifSetupError;
use crate::model::SemifModelTier;
use crate::process::{command_output, local_command_output, run_command};
use crate::{SEMIF_REPOSITORY, SEMIF_SOURCE_REVISION};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const SEMIF_SETUP_RECEIPT_SCHEMA_VERSION: u32 = 1;
const SEMIF_SETUP_RECEIPT_SUFFIX: &str = "belief-lab-setup.json";

pub fn semif_score_path(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join(".venv").join("Scripts").join("semif-score.exe")
    } else {
        root.join(".venv").join("bin").join("semif-score")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemifInstallBackend {
    Torch,
    Mlx,
    LlamaCpp,
}

impl SemifInstallBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "torch" => Some(Self::Torch),
            "mlx" => Some(Self::Mlx),
            "llamacpp" => Some(Self::LlamaCpp),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Torch => "torch",
            Self::Mlx => "mlx",
            Self::LlamaCpp => "llamacpp",
        }
    }

    fn pip_extra(self) -> &'static str {
        match self {
            Self::Torch => ".",
            Self::Mlx => ".[mlx]",
            Self::LlamaCpp => ".[llamacpp]",
        }
    }

    fn import_probe(self) -> &'static str {
        match self {
            Self::Torch => "import torch",
            Self::Mlx => "import mlx.core; import mlx_lm",
            Self::LlamaCpp => "import llama_cpp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemifSetupOutcome {
    pub cloned: bool,
    pub venv_created: bool,
    pub executable: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SemifSetupReceipt {
    schema_version: u32,
    semif_repository: String,
    semif_revision: String,
    backend: String,
}

impl SemifSetupReceipt {
    fn new(backend: SemifInstallBackend) -> Self {
        Self {
            schema_version: SEMIF_SETUP_RECEIPT_SCHEMA_VERSION,
            semif_repository: SEMIF_REPOSITORY.to_string(),
            semif_revision: SEMIF_SOURCE_REVISION.to_string(),
            backend: backend.as_str().to_string(),
        }
    }
}

fn setup_receipt_path(directory: &Path) -> PathBuf {
    let directory_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("semif");
    directory.with_file_name(format!("{directory_name}.{SEMIF_SETUP_RECEIPT_SUFFIX}"))
}

fn setup_receipt_matches(directory: &Path, backend: SemifInstallBackend) -> bool {
    fs::read_to_string(setup_receipt_path(directory))
        .ok()
        .and_then(|contents| serde_json::from_str::<SemifSetupReceipt>(&contents).ok())
        .is_some_and(|receipt| receipt == SemifSetupReceipt::new(backend))
}

fn write_setup_receipt(
    directory: &Path,
    backend: SemifInstallBackend,
) -> Result<(), SemifSetupError> {
    let path = setup_receipt_path(directory);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".belief-lab-semif-setup-")
        .tempfile_in(parent)
        .map_err(|error| SemifSetupError::Io {
            context: format!("create setup receipt beside {}", path.display()),
            message: error.to_string(),
        })?;

    serde_json::to_writer_pretty(temporary.as_file_mut(), &SemifSetupReceipt::new(backend))
        .map_err(|error| SemifSetupError::Io {
            context: format!("serialize {}", path.display()),
            message: error.to_string(),
        })?;
    temporary
        .as_file_mut()
        .write_all(b"\n")
        .and_then(|()| temporary.as_file_mut().sync_all())
        .map_err(|error| SemifSetupError::Io {
            context: format!("write {}", path.display()),
            message: error.to_string(),
        })?;
    temporary
        .persist(&path)
        .map_err(|error| SemifSetupError::Io {
            context: format!("replace {}", path.display()),
            message: error.error.to_string(),
        })?;
    Ok(())
}

pub fn semif_install_is_ready(directory: &Path, backend: SemifInstallBackend) -> bool {
    let interpreter = venv_python(directory);
    checkout_matches_pin(directory)
        && venv_is_usable(&interpreter)
        && backend_is_usable(&interpreter, backend)
        && semif_score_path(directory).is_file()
        && setup_receipt_matches(directory, backend)
}

pub fn bootstrap_semif(
    directory: &Path,
    backend: SemifInstallBackend,
) -> Result<SemifSetupOutcome, SemifSetupError> {
    let cloned = if directory.exists() {
        if !directory.join(".git").is_dir() {
            return Err(SemifSetupError::ExistingPathIsNotRepository(
                directory.to_path_buf(),
            ));
        }
        let origin = command_output(
            Command::new("git")
                .arg("-C")
                .arg(directory)
                .arg("remote")
                .arg("get-url")
                .arg("origin"),
            "read SemIf origin",
        )?;
        if normalize_repository_url(&origin) != normalize_repository_url(SEMIF_REPOSITORY) {
            return Err(SemifSetupError::UnexpectedRepositoryOrigin {
                directory: directory.to_path_buf(),
                origin,
            });
        }
        false
    } else {
        if let Some(parent) = directory.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| SemifSetupError::Io {
                    context: format!("create {}", parent.display()),
                    message: error.to_string(),
                })?;
            }
        }
        run_command(
            Command::new("git")
                .arg("clone")
                .arg("--no-checkout")
                .arg(SEMIF_REPOSITORY)
                .arg(directory),
            "clone SemIf",
        )?;
        true
    };

    if !cloned {
        ensure_clean_checkout(directory)?;
    }

    if !revision_exists(directory)? {
        run_command(
            Command::new("git")
                .arg("-C")
                .arg(directory)
                .arg("fetch")
                .arg("origin")
                .arg("master"),
            "fetch pinned SemIf revision",
        )?;
    }

    run_command(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("checkout")
            .arg("--detach")
            .arg(SEMIF_SOURCE_REVISION),
        "check out pinned SemIf revision",
    )?;
    ensure_clean_checkout(directory)?;

    let installation_ready = semif_install_is_ready(directory, backend);
    let interpreter = venv_python(directory);
    let venv_created = if venv_is_usable(&interpreter) {
        false
    } else {
        let python = env::var("BELIEF_SEMIF_PYTHON").unwrap_or_else(|_| "python3".into());
        let venv = directory.join(".venv");
        let mut command = Command::new(&python);
        command.arg("-m").arg("venv");
        if venv.exists() {
            command.arg("--clear");
        }
        command.arg(&venv);
        run_command(&mut command, "create or repair SemIf virtual environment")?;
        true
    };

    // Preserve the interpreter symlink: resolving it escapes the virtual environment.
    let interpreter = install_interpreter(directory).map_err(|error| SemifSetupError::Io {
        context: "resolve SemIf virtual-environment interpreter".into(),
        message: error.to_string(),
    })?;

    let executable = semif_score_path(directory);
    if !installation_ready || venv_created {
        // SemIf requires Torch for tokenizer plumbing even with llama.cpp. Use CPU wheels
        // on Linux to avoid downloading the unrelated CUDA runtime for the CPU setup.
        if backend == SemifInstallBackend::LlamaCpp && cfg!(target_os = "linux") {
            run_command(
                Command::new(&interpreter).args([
                    "-m",
                    "pip",
                    "install",
                    "torch==2.10.0",
                    "--index-url",
                    "https://download.pytorch.org/whl/cpu",
                ]),
                "install CPU Torch",
            )?;
        }
        run_command(
            Command::new(&interpreter)
                .current_dir(directory)
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("-e")
                .arg(backend.pip_extra()),
            "install pinned SemIf",
        )?;

        if !executable.is_file() {
            return Err(SemifSetupError::MissingExecutable(executable));
        }
        write_setup_receipt(directory, backend)?;
    }

    Ok(SemifSetupOutcome {
        cloned,
        venv_created,
        executable,
    })
}

fn ensure_clean_checkout(directory: &Path) -> Result<(), SemifSetupError> {
    let status = command_output(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("status")
            .arg("--porcelain")
            .arg("--untracked-files=all"),
        "inspect SemIf working tree",
    )?;
    if status.is_empty() {
        Ok(())
    } else {
        Err(SemifSetupError::DirtyCheckout {
            directory: directory.to_path_buf(),
            status,
        })
    }
}

fn venv_is_usable(interpreter: &Path) -> bool {
    if !interpreter.is_file() {
        return false;
    }

    let pip_available = Command::new(interpreter)
        .arg("-m")
        .arg("pip")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    let dependencies_consistent = Command::new(interpreter)
        .arg("-m")
        .arg("pip")
        .arg("check")
        .output()
        .is_ok_and(|output| output.status.success());

    pip_available && dependencies_consistent
}

fn backend_is_usable(interpreter: &Path, backend: SemifInstallBackend) -> bool {
    Command::new(interpreter)
        .arg("-c")
        .arg(backend.import_probe())
        .output()
        .is_ok_and(|output| output.status.success())
}

fn checkout_matches_pin(directory: &Path) -> bool {
    if !directory.join(".git").is_dir() {
        return false;
    }

    let origin = local_command_output(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("remote")
            .arg("get-url")
            .arg("origin"),
    );
    let head = local_command_output(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("rev-parse")
            .arg("HEAD"),
    );
    let status = local_command_output(
        Command::new("git")
            .arg("-C")
            .arg(directory)
            .arg("status")
            .arg("--porcelain")
            .arg("--untracked-files=all"),
    );

    match (origin, head, status) {
        (Some(origin), Some(head), Some(status)) => {
            checkout_state_matches_pin(&origin, &head, &status)
        }
        _ => false,
    }
}

fn checkout_state_matches_pin(origin: &str, head: &str, status: &str) -> bool {
    normalize_repository_url(origin) == normalize_repository_url(SEMIF_REPOSITORY)
        && head.trim() == SEMIF_SOURCE_REVISION
        && status.trim().is_empty()
}

fn revision_exists(directory: &Path) -> Result<bool, SemifSetupError> {
    let status = Command::new("git")
        .arg("-C")
        .arg(directory)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{SEMIF_SOURCE_REVISION}^{{commit}}"))
        .status()
        .map_err(|error| SemifSetupError::Command {
            context: "check pinned SemIf revision".into(),
            message: error.to_string(),
        })?;
    Ok(status.success())
}

fn normalize_repository_url(value: &str) -> String {
    value
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_ascii_lowercase()
}

fn install_interpreter(directory: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(venv_python(directory))
}

/// Acquire only the pinned tokenizer, first attempting an entirely local cache load.
pub fn prepare_tokenizer(tier: SemifModelTier, directory: &Path) -> Result<(), SemifSetupError> {
    run_command(Command::new(venv_python(directory)).arg("-c").arg(
        "import sys
from transformers import AutoTokenizer
try:
 AutoTokenizer.from_pretrained(sys.argv[1], revision=sys.argv[2], local_files_only=True, trust_remote_code=False)
except OSError:
 AutoTokenizer.from_pretrained(sys.argv[1], revision=sys.argv[2], trust_remote_code=False)"
    ).arg(tier.pin().source).arg(tier.pin().source_revision), "prepare pinned tokenizer")
}

fn venv_python(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join(".venv").join("Scripts").join("python.exe")
    } else {
        root.join(".venv").join("bin").join("python")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn installation_preserves_virtual_environment_interpreter_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("semif");
        let python = venv_python(&root);
        fs::create_dir_all(python.parent().unwrap()).unwrap();
        let target = temp.path().join("system-python");
        fs::write(&target, b"system interpreter").unwrap();
        std::os::unix::fs::symlink(&target, &python).unwrap();
        let executable = install_interpreter(&root).unwrap();
        assert_eq!(executable, python);
        assert_ne!(executable, fs::canonicalize(&python).unwrap());
    }

    #[test]
    fn unusable_venv_is_not_treated_as_complete() {
        let temp = tempfile::tempdir().unwrap();
        let fake = temp.path().join("python");
        fs::write(&fake, b"not an executable").unwrap();

        assert!(!venv_is_usable(&fake));
    }

    #[test]
    fn dirty_checkout_status_is_reported() {
        let temp = tempfile::tempdir().unwrap();
        run_command(
            Command::new("git").arg("init").arg(temp.path()),
            "initialize test repository",
        )
        .unwrap();
        fs::write(temp.path().join("untracked.txt"), b"change").unwrap();

        let error = ensure_clean_checkout(temp.path()).unwrap_err();
        assert!(matches!(error, SemifSetupError::DirtyCheckout { .. }));
    }

    #[test]
    fn install_backend_parsing_is_stable() {
        assert_eq!(
            SemifInstallBackend::parse("llamacpp"),
            Some(SemifInstallBackend::LlamaCpp)
        );
        assert_eq!(SemifInstallBackend::parse("unknown"), None);
    }

    #[test]
    fn checkout_readiness_requires_exact_clean_pinned_state() {
        assert!(checkout_state_matches_pin(
            SEMIF_REPOSITORY,
            SEMIF_SOURCE_REVISION,
            ""
        ));
        assert!(!checkout_state_matches_pin(
            SEMIF_REPOSITORY,
            "0000000000000000000000000000000000000000",
            ""
        ));
        assert!(!checkout_state_matches_pin(
            SEMIF_REPOSITORY,
            SEMIF_SOURCE_REVISION,
            " M src/semif_phase1/cli.py"
        ));
        assert!(!checkout_state_matches_pin(
            "https://example.invalid/semif.git",
            SEMIF_SOURCE_REVISION,
            ""
        ));
    }

    #[test]
    fn backend_readiness_probes_backend_specific_imports() {
        assert_eq!(SemifInstallBackend::Torch.import_probe(), "import torch");
        assert_eq!(
            SemifInstallBackend::Mlx.import_probe(),
            "import mlx.core; import mlx_lm"
        );
        assert_eq!(
            SemifInstallBackend::LlamaCpp.import_probe(),
            "import llama_cpp"
        );
    }

    #[test]
    fn setup_receipt_is_outside_the_semif_checkout() {
        let root = Path::new(".local/semif");

        assert_eq!(
            setup_receipt_path(root),
            Path::new(".local/semif.belief-lab-setup.json")
        );
        assert!(!setup_receipt_path(root).starts_with(root));
    }

    #[test]
    fn setup_receipt_binds_revision_and_backend() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("semif");
        fs::create_dir(&root).unwrap();

        write_setup_receipt(&root, SemifInstallBackend::LlamaCpp).unwrap();

        assert!(setup_receipt_matches(&root, SemifInstallBackend::LlamaCpp));
        assert!(!setup_receipt_matches(&root, SemifInstallBackend::Torch));

        fs::write(
            setup_receipt_path(&root),
            format!(
                r#"{{"schemaVersion":1,"semifRepository":"{SEMIF_REPOSITORY}","semifRevision":"stale","backend":"llamacpp"}}"#
            ),
        )
        .unwrap();
        assert!(!setup_receipt_matches(&root, SemifInstallBackend::LlamaCpp));
    }

    #[test]
    fn invalid_setup_receipt_is_not_reused() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("semif");
        fs::create_dir(&root).unwrap();
        fs::write(setup_receipt_path(&root), b"{not json").unwrap();

        assert!(!setup_receipt_matches(&root, SemifInstallBackend::LlamaCpp));
    }
}
