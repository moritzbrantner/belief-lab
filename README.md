# Belief Lab

Source-grounded investigation of **which people express or hold which beliefs**, without deciding whether the propositions are objectively true. The legacy proposition-level baseline remains available as a separately labeled heuristic, not as an attribution score.

## Compare attributed beliefs

[Open the person-belief comparison](https://moritzbrantner.github.io/belief-lab/people.html) to examine who endorses or disputes one scoped proposition. The example distinguishes **confidence (observation)**, **confidence (person)**, and **conviction**, shows exact *supplied* source passages plus producer revisions, and leaves unsupported values unknown. You can record a new analyst annotation from an imported producer evidence reference.

This first slice works with prepared `belief_evidence_interchange@1` references and separately supplied excerpts/assessments. It does not fetch or verify the original source text, extract source claims, infer calibrated person confidence, create shared identities, or save complete portable investigations. Annotation revisions can be inspected, corrected, withdrawn and exported with their reasons; these remain investigator-supplied history, not a verified record of changes in a person’s mind. The example's numerical assessments are **scripted**, not independently derived; recording a real annotation marks its assessments as analyst-provided. These boundaries are deliberate pending the fuller work under issues #35–#37. See [Workbench](docs/WORKBENCH.md).

## Try the workbench

[Open the evidence workbench](https://moritzbrantner.github.io/belief-lab/) to load examples or local JSON files and run the Rust policy → inference → explanation pipeline in your browser. Browser examples use scripted judgments and identify that explicitly.

For real model-backed analysis in a local browser UI, run from a fresh clone:

```sh
cargo run -- workbench
```

Open the printed URL, choose a model, and click **Run analysis**. The example is preloaded; the first analysis sets up the pinned provider/model. The UI ships inside the Rust executable, so no Node or browser build is needed. See [Workbench and roadmap coverage](docs/WORKBENCH.md) for browser builds, downloadable inputs, and verification.

## Quick start

After cloning the repository, the default command is immediately runnable:

```bash
cargo run
```

That executes a deterministic offline demo of the complete evidence → judgment → policy → inference → store → explanation path. It does **not** require a `.env` file, Python, SemIf, or model weights.

To see what the pipeline does with a given evidence batch, without reading the code:

```bash
cargo run -q -- explain fixtures/explain/semantic-research/evidence.json \
  --judgments fixtures/explain/semantic-research/judgments.json --profile semantic_research
```

Golden outputs for every case in `fixtures/explain/` are checked by `cargo test --workspace`. See [Getting started](docs/GETTING_STARTED.md#explain-an-evidence-batch).

For real local semantic scoring:

```bash
cargo run -- doctor
cargo run -- setup
cargo run -- semantic-demo
```

`setup` is retry-safe and idempotent. By default it installs the pinned SemIf revision under `.local/semif` with the CPU `llama.cpp` backend and downloads the phone-tier GGUF (~639 MB) under `.local/models`. A successful installation records the exact SemIf revision and backend beside the checkout. Re-running the command with a matching healthy environment reuses that receipt and skips `pip install`, so already-prepared provider resources can be reused offline. An incomplete environment, stale/corrupt receipt, or backend change is repaired instead. A dirty SemIf checkout is refused rather than silently attributed to the pinned revision.

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
