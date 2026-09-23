use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semantic_decision::{
    AuthorizedDecisionRequest, DecisionEngineError,
    SemanticDecisionEngine, SemanticDecisionReceipt,
};
use serde::Deserialize;
use serde_json::json;

pub const SEMIF_REPOSITORY: &str =
    "https://github.com/TheoLeeCJ/SemIf.git";
pub const SEMIF_SOURCE_REVISION: &str =
    "1f2dea3e25379f9dfc98cb83c324f00ab5deda37";

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemifModelTier {
    Phone,
    Desktop,
    HighMemory,
}

impl SemifModelTier {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "phone" => Some(Self::Phone),
            "desktop" => Some(Self::Desktop),
            "high-memory" | "high_memory" => {
                Some(Self::HighMemory)
            }
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Desktop => "desktop",
            Self::HighMemory => "high-memory",
        }
    }

    pub fn pin(self) -> SemifModelPin {
        match self {
            Self::Phone => SemifModelPin {
                tier: self,
                source: "Qwen/Qwen3-0.6B",
                source_revision:
                    "c1899de289a04d12100db370d81485cdf75e47ca",
                gguf_repository: "Qwen/Qwen3-0.6B-GGUF",
                gguf_revision:
                    "23749fefcc72300e3a2ad315e1317431b06b590a",
                gguf_file: "Qwen3-0.6B-Q8_0.gguf",
                gguf_bytes: 639_446_688,
            },
            Self::Desktop => SemifModelPin {
                tier: self,
                source: "openbmb/MiniCPM5-2B",
                source_revision:
                    "12a3808a956f869c767195e9266b59c4d21d92e2",
                gguf_repository:
                    "openbmb/MiniCPM5-2B-GGUF",
                gguf_revision:
                    "2079a22f3beaa4e306449978533478fe0522f4b3",
                gguf_file: "MiniCPM5-2B-Q4_K_M.gguf",
                gguf_bytes: 1_561_318_368,
            },
            Self::HighMemory => SemifModelPin {
                tier: self,
                source: "Qwen/Qwen3.5-4B",
                source_revision:
                    "851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a",
                gguf_repository:
                    "bartowski/Qwen_Qwen3.5-4B-GGUF",
                gguf_revision:
                    "4168f45a16a1290d65a4ec0fa312ae917a4c15d6",
                gguf_file:
                    "Qwen_Qwen3.5-4B-Q4_K_M.gguf",
                gguf_bytes: 3_013_027_808,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemifModelPin {
    pub tier: SemifModelTier,
    pub source: &'static str,
    pub source_revision: &'static str,
    pub gguf_repository: &'static str,
    pub gguf_revision: &'static str,
    pub gguf_file: &'static str,
    pub gguf_bytes: u64,
}

impl SemifModelPin {
    pub fn gguf_url(self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.gguf_repository,
            self.gguf_revision,
            self.gguf_file
        )
    }
}

pub fn model_catalog() -> [SemifModelPin; 3] {
    [
        SemifModelTier::Phone.pin(),
        SemifModelTier::Desktop.pin(),
        SemifModelTier::HighMemory.pin(),
    ]
}

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
        Self::new(
            semif_score_path(semif_directory.as_ref()),
            model,
            backend,
        )
    }

    pub fn model(&self) -> SemifModelPin {
        self.model
    }

    pub fn backend(&self) -> &SemifBackend {
        &self.backend
    }

    fn command(
        &self,
        input: &Path,
        output: &Path,
    ) -> Command {
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

        if let SemifBackend::LlamaCpp {
            gguf,
            threads,
        } = &self.backend
        {
            command.arg("--gguf").arg(gguf);
            if let Some(threads) = threads {
                command
                    .arg("--llama-threads")
                    .arg(threads.to_string());
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
        let temp_id =
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "belief-lab-semif-{}-{temp_id}",
            std::process::id()
        );
        let input = std::env::temp_dir()
            .join(format!("{stem}.input.jsonl"));
        let output = std::env::temp_dir()
            .join(format!("{stem}.output.jsonl"));

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
            .map_err(|error| {
                engine_error(
                    "serialize SemIf request",
                    error,
                )
            })?;
        fs::write(&input, format!("{input_json}\n"))
            .map_err(|error| {
                engine_error("write SemIf request", error)
            })?;

        let process = self.command(&input, &output).output();
        let _ = fs::remove_file(&input);
        let process = process.map_err(|error| {
            engine_error("start SemIf", error)
        })?;
        if !process.status.success() {
            let _ = fs::remove_file(&output);
            return Err(DecisionEngineError::new(format!(
                "SemIf exited with {}: {}",
                process.status,
                String::from_utf8_lossy(&process.stderr)
                    .trim()
            )));
        }

        let output_json = fs::read_to_string(&output)
            .map_err(|error| {
                engine_error("read SemIf output", error)
            });
        let _ = fs::remove_file(&output);

        parse_semif_output(
            request,
            self.model,
            self.backend.runtime_name(),
            &output_json?,
        )
        .map_err(|error| {
            DecisionEngineError::new(error.to_string())
        })
    }
}

#[derive(Debug, Deserialize)]
struct SemifOutput {
    id: String,
    option_ids: Vec<String>,
    probabilities: Vec<f64>,
    prompt_sha256: String,
    model: SemifOutputModel,
    readout: String,
}

#[derive(Debug, Deserialize)]
struct SemifOutputModel {
    source: String,
    revision: String,
    #[serde(default)]
    gguf: Option<SemifGgufRecord>,
}

#[derive(Debug, Deserialize)]
struct SemifGgufRecord {
    file: String,
    bytes: u64,
    sha256: String,
}

fn parse_semif_output(
    request: &semantic_decision::DecisionRequest,
    model: SemifModelPin,
    runtime: &str,
    output: &str,
) -> Result<SemanticDecisionReceipt, SemifProviderError> {
    let mut lines =
        output.lines().filter(|line| !line.trim().is_empty());
    let line =
        lines.next().ok_or(SemifProviderError::EmptyOutput)?;
    if lines.next().is_some() {
        return Err(SemifProviderError::MultipleOutputRows);
    }

    let parsed = serde_json::from_str::<SemifOutput>(line)
        .map_err(|error| {
            SemifProviderError::InvalidOutput(
                error.to_string(),
            )
        })?;

    if parsed.id != request.id() {
        return Err(
            SemifProviderError::RequestIdMismatch {
                expected: request.id().to_string(),
                actual: parsed.id,
            },
        );
    }

    let expected_option_ids = request
        .options()
        .iter()
        .map(|option| option.id().to_string())
        .collect::<Vec<_>>();
    if parsed.option_ids != expected_option_ids {
        return Err(
            SemifProviderError::OptionOrderMismatch,
        );
    }

    if parsed.probabilities.len()
        != parsed.option_ids.len()
    {
        return Err(
            SemifProviderError::ProbabilityCountMismatch,
        );
    }

    if parsed.model.source != model.source
        || parsed.model.revision != model.source_revision
    {
        return Err(
            SemifProviderError::ModelIdentityMismatch {
                expected_source: model.source.to_string(),
                expected_revision:
                    model.source_revision.to_string(),
                actual_source: parsed.model.source,
                actual_revision: parsed.model.revision,
            },
        );
    }

    let model_revision = if runtime == "llamacpp" {
        let gguf = parsed
            .model
            .gguf
            .ok_or(
                SemifProviderError::MissingGgufIdentity,
            )?;
        if gguf.file != model.gguf_file
            || gguf.bytes != model.gguf_bytes
            || gguf.sha256.len() != 64
        {
            return Err(
                SemifProviderError::GgufIdentityMismatch,
            );
        }
        format!(
            "{}+gguf-sha256:{}",
            model.source_revision,
            gguf.sha256
        )
    } else {
        model.source_revision.to_string()
    };

    let scores = parsed
        .option_ids
        .into_iter()
        .zip(parsed.probabilities)
        .collect::<BTreeMap<_, _>>();

    SemanticDecisionReceipt::new(
        request,
        "semif",
        SEMIF_SOURCE_REVISION,
        model.source,
        model_revision,
        runtime,
        parsed.prompt_sha256,
        parsed.readout,
        scores,
    )
    .map_err(|error| {
        SemifProviderError::InvalidReceipt(
            error.to_string(),
        )
    })
}

pub fn semif_score_path(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join(".venv")
            .join("Scripts")
            .join("semif-score.exe")
    } else {
        root.join(".venv")
            .join("bin")
            .join("semif-score")
    }
}

fn engine_error(
    context: &str,
    error: impl std::fmt::Display,
) -> DecisionEngineError {
    DecisionEngineError::new(format!(
        "could not {context}: {error}"
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SemifProviderError {
    EmptyOutput,
    MultipleOutputRows,
    InvalidOutput(String),
    RequestIdMismatch {
        expected: String,
        actual: String,
    },
    OptionOrderMismatch,
    ProbabilityCountMismatch,
    MissingGgufIdentity,
    GgufIdentityMismatch,
    ModelIdentityMismatch {
        expected_source: String,
        expected_revision: String,
        actual_source: String,
        actual_revision: String,
    },
    InvalidReceipt(String),
}

impl std::fmt::Display for SemifProviderError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::EmptyOutput => {
                f.write_str(
                    "SemIf produced no result row",
                )
            }
            Self::MultipleOutputRows => {
                f.write_str(
                    "SemIf produced more than one result for one decision",
                )
            }
            Self::InvalidOutput(error) => {
                write!(
                    f,
                    "invalid SemIf output: {error}"
                )
            }
            Self::RequestIdMismatch {
                expected,
                actual,
            } => write!(
                f,
                "SemIf result id {actual:?} does not match {expected:?}"
            ),
            Self::OptionOrderMismatch => {
                f.write_str(
                    "SemIf result option ids do not match the authorized request",
                )
            }
            Self::ProbabilityCountMismatch => {
                f.write_str(
                    "SemIf result probability count does not match its option ids",
                )
            }
            Self::MissingGgufIdentity => {
                f.write_str(
                    "llama.cpp SemIf result did not report its GGUF identity",
                )
            }
            Self::GgufIdentityMismatch => {
                f.write_str(
                    "llama.cpp SemIf result GGUF identity does not match the pinned model artifact",
                )
            }
            Self::ModelIdentityMismatch {
                expected_source,
                expected_revision,
                actual_source,
                actual_revision,
            } => write!(
                f,
                "SemIf used model {actual_source}@{actual_revision}, expected {expected_source}@{expected_revision}"
            ),
            Self::InvalidReceipt(error) => {
                write!(
                    f,
                    "invalid SemIf decision receipt: {error}"
                )
            }
        }
    }
}

impl std::error::Error for SemifProviderError {}

#[cfg(test)]
mod tests {
    use belief_core::{
        EntityId, EvidenceClass, EvidenceFamilyId,
        EvidenceId, EvidencePurpose, EvidenceRef,
        InferenceClass, ProducerRef, Provenance,
        SourceRef,
    };
    use belief_policy::PolicyConfig;
    use semantic_decision::{
        DecisionEvidenceUse, DecisionOption,
        DecisionRequest,
    };
    use serde_json::Value;

    use super::*;

    fn request(
    ) -> semantic_decision::AuthorizedDecisionRequest {
        let evidence = EvidenceRef::new(
            EvidenceId::new("evidence:1").unwrap(),
            Some(
                EntityId::new("person:alice").unwrap(),
            ),
            EvidenceClass::Transcript,
            EvidenceFamilyId::new("family:1").unwrap(),
            None,
            Provenance::new(
                SourceRef::new(
                    "youtube-corpus",
                    "video:1",
                    "segment:1",
                    "sha256:source",
                )
                .unwrap(),
                ProducerRef::new(
                    "audio-analysis",
                    "commit:1",
                    None,
                    None,
                )
                .unwrap(),
                [],
            ),
        );
        let request = DecisionRequest::new(
            "decision:1",
            Value::String(
                "bounded transcript".into(),
            ),
            "Does this support the proposition?",
            vec![
                DecisionOption::new(
                    "supports",
                    "Supports",
                )
                .unwrap(),
                DecisionOption::new(
                    "contradicts",
                    "Contradicts",
                )
                .unwrap(),
                DecisionOption::new(
                    "unknown",
                    "Unknown",
                )
                .unwrap(),
            ],
            InferenceClass::Descriptive,
            vec![DecisionEvidenceUse::new(
                evidence,
                EvidencePurpose::Corroboration,
            )],
        )
        .unwrap();

        request
            .authorize(
                &PolicyConfig::from_pairs([(
                    "BELIEF_POLICY_PROFILE",
                    "semantic_research",
                )])
                .unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn catalog_uses_immutable_semif_model_revisions() {
        for pin in model_catalog() {
            assert_eq!(pin.source_revision.len(), 40);
            assert_eq!(pin.gguf_revision.len(), 40);
            assert!(pin.gguf_bytes > 0);
            assert!(
                pin.gguf_url()
                    .contains(pin.gguf_revision)
            );
        }
    }

    #[test]
    fn parses_pinned_direct_semif_output_into_receipt() {
        let authorized = request();
        let pin = SemifModelTier::Phone.pin();
        let output = format!(
            r#"{{"id":"decision:1","option_ids":["supports","contradicts","unknown"],"probabilities":[0.7,0.2,0.1],"prompt_sha256":"abc123","model":{{"source":"{}","revision":"{}","gguf":{{"file":"{}","bytes":{},"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}},"readout":"native-full-vocabulary-last-position"}}"#,
            pin.source,
            pin.source_revision,
            pin.gguf_file,
            pin.gguf_bytes
        );

        let receipt = parse_semif_output(
            authorized.request(),
            pin,
            "llamacpp",
            &output,
        )
        .unwrap();

        assert_eq!(receipt.provider(), "semif");
        assert_eq!(
            receipt.provider_revision(),
            SEMIF_SOURCE_REVISION
        );
        assert_eq!(receipt.model(), pin.source);
        assert_eq!(receipt.runtime(), "llamacpp");
        assert_eq!(
            receipt.selected_option(),
            "supports"
        );
        assert!(
            receipt
                .model_revision()
                .contains("gguf-sha256:")
        );
    }

    #[test]
    fn rejects_model_identity_drift() {
        let authorized = request();
        let pin = SemifModelTier::Phone.pin();
        let output = r#"{"id":"decision:1","option_ids":["supports","contradicts","unknown"],"probabilities":[0.7,0.2,0.1],"prompt_sha256":"abc123","model":{"source":"other/model","revision":"0000000000000000000000000000000000000000"},"readout":"direct"}"#;

        assert!(matches!(
            parse_semif_output(
                authorized.request(),
                pin,
                "torch",
                output
            ),
            Err(
                SemifProviderError::ModelIdentityMismatch {
                    ..
                }
            )
        ));
    }
}
