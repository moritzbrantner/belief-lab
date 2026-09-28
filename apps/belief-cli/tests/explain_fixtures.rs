//! Golden-output checks for `belief explain` over `fixtures/explain/<case>/`.
//!
//! Each case directory has a `case.json` listing runs. Every run executes the real binary as
//! `belief explain <args>` from the case directory, once as text and once with `--json`, and
//! compares stdout byte-for-byte with `expected/<run>.txt` and `expected/<run>.json`. The exit
//! code must match the run's `exit` and stderr must be empty.
//!
//! After an intentional output change, regenerate the expectations and review the diff:
//!
//! ```sh
//! BELIEF_UPDATE_FIXTURES=1 cargo test -p belief-cli --test explain_fixtures
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

const UPDATE_ENV: &str = "BELIEF_UPDATE_FIXTURES";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    #[allow(dead_code)] // Documentation for readers of the fixture.
    description: String,
    runs: Vec<Run>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    name: String,
    args: Vec<String>,
    exit: i32,
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/explain")
}

fn case_dirs() -> Vec<PathBuf> {
    let mut dirs = fs::read_dir(fixtures_root())
        .expect("fixtures/explain must exist")
        .map(|entry| entry.expect("readable fixture entry").path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn run_explain(case_dir: &Path, args: &[String]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_belief"))
        .arg("explain")
        .args(args)
        .current_dir(case_dir)
        .output()
        .expect("belief binary runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    )
}

#[test]
fn explain_fixtures_match_expected_output() {
    let update = std::env::var_os(UPDATE_ENV).is_some_and(|value| value == "1");
    let mut failures = Vec::new();
    let cases = case_dirs();
    assert!(!cases.is_empty(), "no explain fixtures found");

    for case_dir in cases {
        let case_name = case_dir.file_name().unwrap().to_string_lossy().into_owned();
        let case: Case = serde_json::from_str(
            &fs::read_to_string(case_dir.join("case.json"))
                .unwrap_or_else(|error| panic!("{case_name}/case.json: {error}")),
        )
        .unwrap_or_else(|error| panic!("{case_name}/case.json: {error}"));
        assert!(!case.runs.is_empty(), "{case_name} has no runs");

        let expected_dir = case_dir.join("expected");
        let mut produced = BTreeSet::new();

        for run in &case.runs {
            for json in [false, true] {
                let mut args = run.args.clone();
                if json {
                    args.push("--json".into());
                }
                let (exit, stdout, stderr) = run_explain(&case_dir, &args);
                let file = format!("{}.{}", run.name, if json { "json" } else { "txt" });
                let label = format!("{case_name}/{file}");
                assert!(produced.insert(file.clone()), "duplicate run {label}");

                if exit != run.exit {
                    failures.push(format!(
                        "{label}: exit {exit}, expected {}; stderr: {stderr}",
                        run.exit
                    ));
                }
                if !stderr.is_empty() {
                    failures.push(format!("{label}: unexpected stderr: {stderr}"));
                }

                let path = expected_dir.join(&file);
                if update {
                    fs::create_dir_all(&expected_dir).unwrap();
                    fs::write(&path, &stdout).unwrap();
                    continue;
                }
                match fs::read_to_string(&path) {
                    Ok(expected) if expected == stdout => {}
                    Ok(expected) => failures.push(format!(
                        "{label}: output differs from expected\n{}",
                        first_difference(&expected, &stdout)
                    )),
                    Err(error) => failures.push(format!("{label}: {error}")),
                }
            }
        }

        if let Ok(entries) = fs::read_dir(&expected_dir) {
            for entry in entries {
                let path = entry.unwrap().path();
                let file = path.file_name().unwrap().to_string_lossy().into_owned();
                if !produced.contains(&file) {
                    if update {
                        fs::remove_file(&path).unwrap();
                    } else {
                        failures.push(format!("{case_name}/expected/{file}: no run produces it"));
                    }
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} explain fixture mismatch(es); if the change is intended, rerun with {UPDATE_ENV}=1 \
         and review the diff:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn first_difference(expected: &str, actual: &str) -> String {
    let expected_lines = expected.lines().collect::<Vec<_>>();
    let actual_lines = actual.lines().collect::<Vec<_>>();
    let line = expected_lines
        .iter()
        .zip(&actual_lines)
        .position(|(expected, actual)| expected != actual)
        .unwrap_or(expected_lines.len().min(actual_lines.len()));
    format!(
        "first difference at line {}:\n  expected: {}\n  actual:   {}",
        line + 1,
        expected_lines.get(line).unwrap_or(&"<end of output>"),
        actual_lines.get(line).unwrap_or(&"<end of output>")
    )
}
