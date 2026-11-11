use crate::model::SemifModelPin;
use crate::output::parse_semif_output;
use crate::setup::semif_score_path;
use semantic_decision::{
    AuthorizedDecisionRequest, DecisionEngineError, SemanticDecisionEngine, SemanticDecisionReceipt,
};
use serde_json::json;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use wait_timeout::ChildExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemifBackend {
    Torch,
    Mlx,
    LlamaCpp {
        gguf: PathBuf,
        threads: Option<usize>,
    },
}

impl SemifBackend {
    pub fn runtime_name(&self) -> &'static str {
        match self {
            Self::Torch => "torch",
            Self::Mlx => "mlx",
            Self::LlamaCpp { .. } => "llamacpp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemifProvider {
    executable: PathBuf,
    model: SemifModelPin,
    backend: SemifBackend,
}

impl SemifProvider {
    pub fn new(
        executable: impl Into<PathBuf>,
        model: SemifModelPin,
        backend: SemifBackend,
    ) -> Self {
        Self {
            executable: executable.into(),
            model,
            backend,
        }
    }

    pub fn from_bootstrap(
        semif_directory: impl AsRef<Path>,
        model: SemifModelPin,
        backend: SemifBackend,
    ) -> Self {
        Self::new(semif_score_path(semif_directory.as_ref()), model, backend)
    }

    pub fn model(&self) -> SemifModelPin {
        self.model
    }

    pub fn backend(&self) -> &SemifBackend {
        &self.backend
    }

    fn command(&self, input: &Path, output: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .arg("--mode")
            .arg("direct")
            .arg("--backend")
            .arg(self.backend.runtime_name())
            .arg("--model")
            .arg(self.model.source)
            .arg("--revision")
            .arg(self.model.source_revision)
            .arg("--input")
            .arg(input)
            .arg("--output")
            .arg(output);

        if let SemifBackend::LlamaCpp { gguf, threads } = &self.backend {
            command.env("HF_HUB_OFFLINE", "1").arg("--gguf").arg(gguf);
            if let Some(threads) = threads {
                command.arg("--llama-threads").arg(threads.to_string());
            }
        }

        command
    }
}

impl SemanticDecisionEngine for SemifProvider {
    fn name(&self) -> &'static str {
        "semif"
    }

    fn decide(
        &self,
        authorized: &AuthorizedDecisionRequest,
    ) -> Result<SemanticDecisionReceipt, DecisionEngineError> {
        let request = authorized.request();
        let exchange = tempfile::Builder::new()
            .prefix("belief-lab-semif-")
            .tempdir()
            .map_err(|error| engine_error("create secure SemIf exchange directory", error))?;
        let input = exchange.path().join("input.jsonl");
        let output = exchange.path().join("output.jsonl");

        let row = json!({
            "id": request.id(),
            "state": request.state(),
            "question": request.question(),
            "options": request.options().iter().map(|option| {
                json!({
                    "id": option.id(),
                    "description": option.description(),
                })
            }).collect::<Vec<_>>(),
        });
        let input_json = serde_json::to_string(&row)
            .map_err(|error| engine_error("serialize SemIf request", error))?;
        fs::write(&input, format!("{input_json}\n"))
            .map_err(|error| engine_error("write SemIf request", error))?;

        let process = bounded_output(&mut self.command(&input, &output), Duration::from_secs(600));
        let process = process.map_err(|error| engine_error("start SemIf", error))?;
        if !process.status.success() {
            return Err(DecisionEngineError::new(format!(
                "SemIf exited with {}: {}",
                process.status,
                String::from_utf8_lossy(&process.stderr).trim()
            )));
        }

        let output_json =
            fs::read_to_string(&output).map_err(|error| engine_error("read SemIf output", error));

        parse_semif_output(
            request,
            self.model,
            self.backend.runtime_name(),
            &output_json?,
        )
        .map_err(|error| DecisionEngineError::new(error.to_string()))
    }
}

/// Files keep verbose native diagnostics from filling subprocess pipes while waiting.
fn bounded_output(command: &mut Command, limit: Duration) -> std::io::Result<std::process::Output> {
    use std::io::{Seek, SeekFrom};
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = command
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()?;
    let status = match child.wait_timeout(limit) {
        Ok(Some(status)) => status,
        result => {
            // Always reap the child, including wait errors and timeout.
            let kill = child.kill();
            let wait = child.wait();
            kill?;
            wait?;
            return match result {
                Err(error) => Err(error),
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "SemIf exceeded its 10-minute execution limit",
                )),
            };
        }
    };
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    let mut out = Vec::new();
    let mut err = Vec::new();
    stdout.read_to_end(&mut out)?;
    stderr.read_to_end(&mut err)?;
    Ok(std::process::Output {
        status,
        stdout: out,
        stderr: err,
    })
}

fn engine_error(context: &str, error: impl std::fmt::Display) -> DecisionEngineError {
    DecisionEngineError::new(format!("could not {context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn provider_execution_has_a_finite_deadline() {
        let error = bounded_output(
            Command::new("sh").args(["-c", "exec sleep 5"]),
            Duration::from_millis(20),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }
}
