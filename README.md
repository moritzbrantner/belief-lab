# Belief Lab

Provenance-first experiments for turning heterogeneous evidence into explainable beliefs.

## Quick start

After cloning the repository, the default command is immediately runnable:

```bash
cargo run
```

That executes a deterministic offline demo of the complete evidence → judgment → policy → inference → store → explanation path. It does **not** require a `.env` file, Python, SemIf, or model weights.

For real local semantic scoring:

```bash
cargo run -- doctor
cargo run -- setup
cargo run -- semantic-demo
```

`setup` is retry-safe and idempotent. By default it installs the pinned SemIf revision under `.local/semif` with the CPU `llama.cpp` backend and downloads the phone-tier GGUF (~639 MB) under `.local/models`. Re-running the command reuses a clean valid checkout and usable virtual environment, repairs an incomplete virtual environment, reuses a completed model, and resumes an interrupted model download. A dirty SemIf checkout is refused rather than silently attributed to the pinned revision.

Optional model tiers are `phone`, `desktop`, and `high-memory`:

```bash
cargo run -- setup desktop
cargo run -- semantic-demo desktop
```

See [Getting started](docs/GETTING_STARTED.md) for prerequisites and recovery behavior.

## Architecture

The current foundation separates:

- `belief-core` — evidence, judgments, claims, beliefs, derivations, provenance, and score semantics;
- `belief-policy` — fail-closed authorization over evidence and inference classes;
- `evidence-interchange` — versioned, policy-admitted references to producer-owned evidence;
- `semantic-decision` — provider-neutral, policy-gated semantic judgments with explicit score semantics and receipts;
- `semif-provider` — pinned local SemIf adapter plus source/model bootstrap tooling;
- `inference-core` — the authorization boundary for inference execution;
- `inference-baseline` — deterministic, correlation-aware soft-truth reference inference;
- `belief-store` — explanation, import receipts, and transitive invalidation semantics.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`docs/POLICY.md`](docs/POLICY.md), [`docs/EVIDENCE_INTERCHANGE.md`](docs/EVIDENCE_INTERCHANGE.md), [`docs/SEMANTIC_DECISIONS.md`](docs/SEMANTIC_DECISIONS.md), [`docs/INFERENCE.md`](docs/INFERENCE.md), and [`docs/STORE.md`](docs/STORE.md).

## Safety-first policy boundary

Configuration is supplied through environment variables. See [`.env.example`](.env.example).

The shipped profiles are deliberately bounded:

- `observe_only` — no inference.
- `semantic_research` — non-biometric semantic evidence and low-risk inference.
- `multimodal_research` — may use producer-owned face/voice track references, corpus-local identity links, and cross-source association only when each is separately enabled.

Sensitive-trait inference and real-world identity resolution are not authorized by any shipped profile. They cannot be enabled by changing `.env` alone.

The policy layer does not own detector, ASR, OCR, NER, or scene truth. Those remain in their producer repositories; `belief-lab` consumes provenance-preserving references downstream.

## Local SemIf

The top-level CLI owns the normal setup flow:

```bash
cargo run -- setup
cargo run -- semantic-demo
```

The lower-level `belief-semif` helper remains available for development, but users do not need to know internal crate names. The adapter pins SemIf and model revisions and stores provider/model/prompt provenance with semantic judgments. SemIf option scores remain conditional option probabilities; only Belief Lab inference may produce belief semantics. See [`docs/SEMANTIC_DECISIONS.md`](docs/SEMANTIC_DECISIONS.md) and [`THIRD_PARTY.md`](THIRD_PARTY.md).

## Validate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --quiet
```

Validation runs the same format, Clippy, and unit-test commands through a pinned reusable workflow on pull requests and pushes to `main`.
