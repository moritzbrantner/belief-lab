use belief_cli::decision::{execute, FixtureEngine, ProviderSelection};
use serde_json::{json, Value};
const INPUT: &str = include_str!("../../../examples/decisions/fixture.json");
fn run(input: &str) -> Result<Value, belief_cli::decision::Failure> {
    execute(input, |selection| match selection {
        ProviderSelection::Fixture { scores } => Ok(Box::new(FixtureEngine {
            scores: scores.clone(),
        })),
        _ => panic!("tests must remain offline"),
    })
}
#[test]
fn fixture_decision_derives_a_belief_with_exact_provenance() {
    let a = run(INPUT).unwrap();
    assert_eq!(a, run(INPUT).unwrap());
    assert_eq!(a["judgment"]["semantics"], "conditional_option_probability");
    assert_eq!(a["belief"]["semantics"], "soft_truth");
    assert_eq!(a["belief"]["value"], 0.95);
    assert_eq!(a["decision"]["provider"], "fixture");
    assert_eq!(
        a["explanation"]["evidence"][0]["source"]["repository"],
        "belief-cli-demo"
    );
}
#[test]
fn invalid_and_denied_requests_never_resolve_a_provider() {
    for (field, value, code) in [
        ("class", json!("sensitive_trait"), "policy_denied"),
        (
            "policy",
            json!({"BELIEF_POLICY_PROFILE":"observe_only"}),
            "policy_denied",
        ),
        (
            "policy",
            json!({"BELIEF_UNKNOWN":"true"}),
            "invalid_request",
        ),
        ("options", json!([]), "invalid_request"),
        (
            "evidenceUses",
            json!([{"id":"missing","purpose":"corroboration"}]),
            "invalid_request",
        ),
        ("schemaVersion", json!(2), "unsupported_schema"),
    ] {
        let mut input: Value = serde_json::from_str(INPUT).unwrap();
        input[field] = value;
        let error = execute(&input.to_string(), |_| {
            panic!("provider called before validation")
        })
        .unwrap_err();
        assert_eq!(error.code, code, "{field}");
    }
    assert_eq!(
        execute("{", |_| panic!()).unwrap_err().code,
        "invalid_request"
    );
}
#[test]
fn invalid_scores_fail_and_unknown_produces_no_belief() {
    let mut input: Value = serde_json::from_str(INPUT).unwrap();
    input["provider"]["scores"] = json!({"supports":0.2,"contradicts":0.2,"unknown":0.2});
    assert_eq!(run(&input.to_string()).unwrap_err().code, "provider_failed");
    input["provider"]["scores"] = json!({"supports":0.1,"contradicts":0.1,"unknown":0.8});
    let result = run(&input.to_string()).unwrap();
    assert!(result["belief"].is_null());
    assert_eq!(result["judgment"]["outcome"], "unknown");
}
#[test]
fn cli_has_clean_machine_streams_and_stable_failure_codes() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_belief"))
        .args(["decide", "../../examples/decisions/fixture.json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "ok");
    assert!(output.stderr.is_empty());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_belief"))
        .args(["decide", "missing.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"]["code"], "input_io");
}
