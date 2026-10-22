//! Shared application pipeline for the CLI and browser workbench.
pub mod attribution;
pub mod decision;
pub mod explain;
pub mod judgments;
pub mod pipeline;
pub mod project;
pub mod render;

#[cfg(target_arch = "wasm32")]
mod browser {
    use crate::explain::{explain, ExplainInput};
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn compare_attributions_json(
        evidence: &str,
        attributions: &str,
        policy: &str,
    ) -> Result<String, JsValue> {
        let run = || -> Result<String, Box<dyn std::error::Error>> {
            if evidence.len() > crate::decision::MAX_INPUT_BYTES
                || attributions.len() > crate::decision::MAX_INPUT_BYTES
                || policy.len() > crate::decision::MAX_INPUT_BYTES
            {
                return Err("inputs must be at most 1 MiB each".into());
            }
            let report = crate::attribution::compare(&crate::attribution::ComparisonInput {
                evidence_json: evidence,
                attributions_json: attributions,
                policy: serde_json::from_str(policy)?,
            })?;
            Ok(serde_json::to_string_pretty(&report)?)
        };
        run().map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen]
    pub fn open_project_json(project: &str, policy: &str) -> Result<String, JsValue> {
        let run = || -> Result<String, Box<dyn std::error::Error>> {
            if project.len() > 4 * crate::decision::MAX_INPUT_BYTES
                || policy.len() > crate::decision::MAX_INPUT_BYTES
            {
                return Err("a project must be at most 4 MiB".into());
            }
            let report = crate::project::open_project(project, &serde_json::from_str(policy)?)?;
            Ok(serde_json::to_string_pretty(&report)?)
        };
        run().map_err(|error| JsValue::from_str(&error.to_string()))
    }

    #[wasm_bindgen]
    pub fn explain_json(evidence: &str, judgments: &str, policy: &str) -> Result<String, JsValue> {
        let run = || -> Result<String, Box<dyn std::error::Error>> {
            if evidence.len() > crate::decision::MAX_INPUT_BYTES
                || judgments.len() > crate::decision::MAX_INPUT_BYTES
                || policy.len() > crate::decision::MAX_INPUT_BYTES
            {
                return Err("inputs must be at most 1 MiB each".into());
            }
            let report = explain(&ExplainInput {
                evidence_json: evidence,
                judgments_json: if judgments.trim().is_empty() {
                    None
                } else {
                    Some(judgments)
                },
                policy: serde_json::from_str(policy)?,
            })?;
            Ok(serde_json::to_string_pretty(&report)?)
        };
        run().map_err(|error| JsValue::from_str(&error.to_string()))
    }
}
