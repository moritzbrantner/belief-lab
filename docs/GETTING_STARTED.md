# Getting started

Belief Lab has two startup levels.

## 1. Offline core demo

Requirements:

- Rust/Cargo.

From the repository root:

```bash
cargo run
```

This is the default command. It constructs a provenance-bearing transcript reference, a supporting judgment, a policy-authorized inference request, a soft-truth belief, stores the result, and reads the explanation back from the store.

It intentionally needs no network access after Rust dependencies are available, no `.env`, no Python environment, and no model weights.

## Explain an evidence batch

`explain` runs any evidence-interchange batch offline through the same path as the demo: interchange validation, policy admission, deterministic judgments, policy authorization, baseline inference, the store, and the store's explanation.

```bash
cargo run -q -- explain fixtures/explain/semantic-research/evidence.json \
  --judgments fixtures/explain/semantic-research/judgments.json \
  --profile semantic_research
```

It prints the admitted and rejected evidence with the policy reason, each requested inference with its status (`derived`, `refused`, `not run`, `no belief`), derived beliefs with score semantics, which judgments the baseline selected or ignored as correlated, and the provenance chain read back from the store. `--json` prints the full structured report. Output has no timestamps or absolute paths and a fixed order.

- `--profile` selects a shipped profile (`observe_only` is the default). `--policy KEY=VALUE` sets any other key from [`.env.example`](../.env.example), such as `BELIEF_BIOMETRIC_EVIDENCE=reference_only`. The same fail-closed limits apply as for `.env`, so sensitive-trait inference and real-world identity resolution stay refused. The process environment is ignored.
- Without `--judgments` only admission runs. A judgment script (`belief_judgment_script@1`, see `fixtures/explain/*/judgments.json`) records the judgments a provider such as SemIf would return: the proposition and inference class, and for each judgment its outcome, `modelConfidence`, correlation group and evidence uses with a purpose. No model runs. Everything after the judgments is the real policy, inference and store code.
- Import is all-or-nothing: one rejected record refuses the whole batch, and the report still lists every record's decision.
- Exit codes: 0 means an explanation was produced, including policy refusals. 2 means an input was malformed or unsupported and nothing was imported. 1 means a usage or I/O error.

Every directory in `fixtures/explain/` is a case. Its `case.json` lists runs, and each run has arguments relative to the case directory and an expected exit code. `cargo test --workspace` runs each case as text and as `--json` and compares the output with `expected/<run>.txt` and `expected/<run>.json`. After an intended output change, regenerate the expected files and review the diff:

```bash
BELIEF_UPDATE_FIXTURES=1 cargo test -p belief-cli --test explain_fixtures
git diff fixtures/explain
```

## 2. Local semantic decisions

The optional SemIf-backed path adds:

- Git;
- Python 3.10+ with `venv`;
- `curl`;
- a C/C++ build toolchain for llama-cpp-python (on Debian/Ubuntu: `build-essential` and `cmake`);
- network access for the initial SemIf/Python/model downloads;
- enough disk space for the selected model and its Python environment.

Check the machine without changing anything:

```bash
cargo run -- doctor
```

Prepare the default CPU-local setup:

```bash
cargo run -- setup
```

This installs the pinned SemIf source revision into `.local/semif`, creates/reuses its virtual environment, installs the `llama.cpp` backend (with CPU Torch wheels on Linux), and downloads the phone-tier GGUF into `.local/models`. It also prepares the exact pinned tokenizer in the Hugging Face cache. Scoring then runs with Hugging Face offline mode. Initial native compilation may take several minutes; setup streams progress to stderr.

Then run an actual model-backed decision through the Belief Lab boundary:

```bash
cargo run -- semantic-demo
```

The semantic demo keeps the normal architecture intact: SemIf produces a provenance-bearing judgment, while Belief Lab policy authorizes the evidence/inference class and Belief Lab inference produces the final soft-truth belief.

## Model tiers

The CLI accepts one optional tier:

| Tier | Artifact | Approximate download |
| --- | --- | ---: |
| `phone` | Qwen3 0.6B Q8_0 | 639 MB |
| `desktop` | MiniCPM5 2B Q4_K_M | 1.56 GB |
| `high-memory` | Qwen3.5 4B Q4_K_M | 3.01 GB |

Examples:

```bash
cargo run -- setup desktop
cargo run -- semantic-demo desktop
```

## Retry and recovery behavior

Setup is designed to be rerun safely:

- a valid existing SemIf checkout is reused;
- its origin must still be the expected upstream repository;
- a dirty checkout is rejected rather than silently installing modified code under the pinned revision;
- the checkout is returned to the exact pinned revision;
- a healthy virtual environment is reused, while a partial/broken virtual environment is recreated with `venv --clear`;
- after a successful package install, Belief Lab records the exact SemIf revision and selected backend in an ignored setup receipt beside the checkout;
- when the checkout is still clean at the exact pinned revision, the receipt matches, the selected backend imports successfully, the virtual environment passes `pip check`, and the `semif-score` executable exists, a repeated `setup` skips `pip install` entirely;
- a moved/dirty checkout, missing backend extra, missing/corrupt receipt, stale revision, or different backend is rejected or repaired rather than silently reused;
- a completed model is reused after exact-size and SHA-256 validation;
- a partial model download is resumed;
- an incomplete final model file is moved back to the partial-download path and resumed.

If `.local/semif` exists but is not the expected git checkout, setup stops rather than deleting or overwriting it.

If a model file has an unexpected size and cannot be resumed into the pinned artifact, setup reports the exact path and expected size instead of silently accepting it.

Once the pinned checkout, virtual environment, setup receipt, and model are present, rerunning `cargo run -- setup [tier]` does not need the package index or model host. The setup path remains local unless one of those declared inputs needs acquisition or repair.

## File-driven semantic scoring and browser examples

```sh
cargo run -- decide examples/decisions/preference.json > result.json
cargo run -- decide examples/decisions/fixture.json  # deterministic, no model
```

The first command validates and authorizes before preparing the pinned provider and selected model automatically. The result includes the original request and provider/inference provenance. See [Workbench](WORKBENCH.md) for the Pages examples, browser uploads, and build instructions.

## Commands

```text
cargo run                         offline core demo
cargo run -- demo                 offline core demo
cargo run -- explain <evidence.json> [--judgments <file>] [--profile <name>] [--policy K=V]... [--json]
                                  explain an evidence batch offline
cargo run -- doctor [tier]        inspect prerequisites and local state
cargo run -- setup [tier]         prepare pinned SemIf + model
cargo run -- semantic-demo [tier] run real local semantic scoring
cargo run -- help                 command summary
```

The lower-level `belief-semif` executable remains available for provider development, but it is not required for normal use.
