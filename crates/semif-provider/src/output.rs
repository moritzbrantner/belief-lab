use crate::model::SemifModelPin;
use crate::SEMIF_SOURCE_REVISION;
use semantic_decision::SemanticDecisionReceipt;
use serde::Deserialize;
use std::collections::BTreeMap;

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

pub(crate) fn parse_semif_output(
    request: &semantic_decision::DecisionRequest,
    model: SemifModelPin,
    runtime: &str,
    output: &str,
) -> Result<SemanticDecisionReceipt, SemifProviderError> {
    let mut lines = output.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next().ok_or(SemifProviderError::EmptyOutput)?;
    if lines.next().is_some() {
        return Err(SemifProviderError::MultipleOutputRows);
    }

    let parsed = serde_json::from_str::<SemifOutput>(line)
        .map_err(|error| SemifProviderError::InvalidOutput(error.to_string()))?;

    if parsed.id != request.id() {
        return Err(SemifProviderError::RequestIdMismatch {
            expected: request.id().to_string(),
            actual: parsed.id,
        });
    }

    let expected_option_ids = request
        .options()
        .iter()
        .map(|option| option.id().to_string())
        .collect::<Vec<_>>();
    if parsed.option_ids != expected_option_ids {
        return Err(SemifProviderError::OptionOrderMismatch);
    }

    if parsed.probabilities.len() != parsed.option_ids.len() {
        return Err(SemifProviderError::ProbabilityCountMismatch);
    }

    if parsed.model.source != model.source || parsed.model.revision != model.source_revision {
        return Err(SemifProviderError::ModelIdentityMismatch {
            expected_source: model.source.to_string(),
            expected_revision: model.source_revision.to_string(),
            actual_source: parsed.model.source,
            actual_revision: parsed.model.revision,
        });
    }

    let model_revision = if runtime == "llamacpp" {
        let gguf = parsed
            .model
            .gguf
            .ok_or(SemifProviderError::MissingGgufIdentity)?;
        if gguf.file != model.gguf_file
            || gguf.bytes != model.gguf_bytes
            || gguf.sha256 != model.gguf_sha256
        {
            return Err(SemifProviderError::GgufIdentityMismatch);
        }
        format!(
            "{}+gguf-sha256:{}",
            model.source_revision, model.gguf_sha256
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
    .map_err(|error| SemifProviderError::InvalidReceipt(error.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SemifProviderError {
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId, EvidencePurpose, EvidenceRef,
        InferenceClass, ProducerRef, Provenance, SourceRef,
    };
    use belief_policy::PolicyConfig;
    use semantic_decision::{DecisionEvidenceUse, DecisionOption, DecisionRequest};
    use serde_json::Value;

    use super::*;
    use crate::model::SemifModelTier;

    fn request() -> semantic_decision::AuthorizedDecisionRequest {
        let evidence = EvidenceRef::new(
            EvidenceId::new("evidence:1").unwrap(),
            Some(EntityId::new("person:alice").unwrap()),
            EvidenceClass::Transcript,
            EvidenceFamilyId::new("family:1").unwrap(),
            None,
            Provenance::new(
                SourceRef::new("youtube-corpus", "video:1", "segment:1", "sha256:source").unwrap(),
                ProducerRef::new("audio-analysis", "commit:1", None, None).unwrap(),
                [],
            ),
        );
        let request = DecisionRequest::new(
            "decision:1",
            Value::String("bounded transcript".into()),
            "Does this support the proposition?",
            vec![
                DecisionOption::new("supports", "Supports").unwrap(),
                DecisionOption::new("contradicts", "Contradicts").unwrap(),
                DecisionOption::new("unknown", "Unknown").unwrap(),
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
                &PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")])
                    .unwrap(),
            )
            .unwrap()
    }

    #[test]
    fn parses_pinned_direct_semif_output_into_receipt() {
        let authorized = request();
        let pin = SemifModelTier::Phone.pin();
        let output = format!(
            r#"{{"id":"decision:1","option_ids":["supports","contradicts","unknown"],"probabilities":[0.7,0.2,0.1],"prompt_sha256":"abc123","model":{{"source":"{}","revision":"{}","gguf":{{"file":"{}","bytes":{},"sha256":"{}"}}}},"readout":"native-full-vocabulary-last-position"}}"#,
            pin.source, pin.source_revision, pin.gguf_file, pin.gguf_bytes, pin.gguf_sha256
        );

        let receipt = parse_semif_output(authorized.request(), pin, "llamacpp", &output).unwrap();

        assert_eq!(receipt.provider(), "semif");
        assert_eq!(receipt.provider_revision(), SEMIF_SOURCE_REVISION);
        assert_eq!(receipt.model(), pin.source);
        assert_eq!(receipt.runtime(), "llamacpp");
        assert_eq!(receipt.selected_option(), "supports");
        assert!(receipt.model_revision().contains("gguf-sha256:"));
    }

    #[test]
    fn rejects_gguf_digest_drift_even_when_file_and_size_match() {
        let authorized = request();
        let pin = SemifModelTier::Phone.pin();
        let output = format!(
            r#"{{"id":"decision:1","option_ids":["supports","contradicts","unknown"],"probabilities":[0.7,0.2,0.1],"prompt_sha256":"abc123","model":{{"source":"{}","revision":"{}","gguf":{{"file":"{}","bytes":{},"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}},"readout":"native-full-vocabulary-last-position"}}"#,
            pin.source, pin.source_revision, pin.gguf_file, pin.gguf_bytes
        );

        assert_eq!(
            parse_semif_output(authorized.request(), pin, "llamacpp", &output),
            Err(SemifProviderError::GgufIdentityMismatch)
        );
    }

    #[test]
    fn rejects_model_identity_drift() {
        let authorized = request();
        let pin = SemifModelTier::Phone.pin();
        let output = r#"{"id":"decision:1","option_ids":["supports","contradicts","unknown"],"probabilities":[0.7,0.2,0.1],"prompt_sha256":"abc123","model":{"source":"other/model","revision":"0000000000000000000000000000000000000000"},"readout":"direct"}"#;

        assert!(matches!(
            parse_semif_output(authorized.request(), pin, "torch", output),
            Err(SemifProviderError::ModelIdentityMismatch { .. })
        ));
    }
}
