//! Explicit online acquisition / real-provider acceptance; excluded from deterministic tests.
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
#[ignore = "downloads pinned provider/model resources and runs all three real CPU models"]
fn all_pinned_tiers_execute_file_requests_with_clean_machine_output() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for tier in ["phone", "desktop", "high-memory"] {
        let mut request: Value =
            serde_json::from_str(include_str!("../../../examples/decisions/preference.json"))
                .unwrap();
        request["provider"]["tier"] = tier.into();
        let mut child = Command::new(env!("CARGO_BIN_EXE_belief"))
            .current_dir(&root)
            .args(["decide", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "{tier}: {result}");
        assert_eq!(result["decision"]["provider"], "semif", "{tier}");
        assert_eq!(result["decision"]["selectedOption"], "supports", "{tier}");
        assert_eq!(result["belief"]["semantics"], "soft_truth", "{tier}");
        assert!(result["decision"]["modelRevision"]
            .as_str()
            .unwrap()
            .contains("gguf-sha256:"));
        assert_eq!(
            result["explanation"]["semantic_decisions"][0]["modelRevision"],
            result["decision"]["modelRevision"]
        );
    }
}
