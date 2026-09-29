//! Shared application pipeline for the CLI and browser workbench.
pub mod decision;
pub mod explain;
pub mod judgments;
pub mod pipeline;
pub mod render;

#[cfg(target_arch = "wasm32")]
mod browser {
    use crate::explain::{explain, ExplainInput};
    use wasm_bindgen::prelude::*;

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
