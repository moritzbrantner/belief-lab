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

## 2. Local semantic decisions

The optional SemIf-backed path adds:

- Git;
- Python 3.10+ with `venv`;
- `curl`;
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

This installs the pinned SemIf source revision into `.local/semif`, creates/reuses its virtual environment, installs the `llama.cpp` backend, and downloads the phone-tier GGUF into `.local/models`.

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
- when that receipt, the virtual environment, and the `semif-score` executable still agree, a repeated `setup` skips `pip install` entirely;
- a missing, corrupt, stale-revision, or different-backend receipt triggers package repair rather than silent reuse;
- a completed model is reused after exact-size validation;
- a partial model download is resumed;
- an incomplete final model file is moved back to the partial-download path and resumed.

If `.local/semif` exists but is not the expected git checkout, setup stops rather than deleting or overwriting it.

If a model file has an unexpected size and cannot be resumed into the pinned artifact, setup reports the exact path and expected size instead of silently accepting it.

Once the pinned checkout, virtual environment, setup receipt, and model are present, rerunning `cargo run -- setup [tier]` does not need the package index or model host. The setup path remains local unless one of those declared inputs needs acquisition or repair.

## Commands

```text
cargo run                         offline core demo
cargo run -- demo                 offline core demo
cargo run -- doctor [tier]        inspect prerequisites and local state
cargo run -- setup [tier]         prepare pinned SemIf + model
cargo run -- semantic-demo [tier] run real local semantic scoring
cargo run -- help                 command summary
```

The lower-level `belief-semif` executable remains available for provider development, but it is not required for normal use.
