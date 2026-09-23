# Belief Lab

Provenance-first experiments for turning heterogeneous evidence into explainable beliefs.

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

Belief Lab can run open semantic decisions locally without treating the provider as belief authority:

```bash
cargo run -p semif-provider --bin belief-semif -- catalog
cargo run -p semif-provider --bin belief-semif -- bootstrap .local/semif llamacpp
cargo run -p semif-provider --bin belief-semif -- download-model phone .local/models
```

The adapter pins SemIf and model revisions and stores provider/model/prompt provenance with semantic judgments. SemIf option scores remain conditional option probabilities; only Belief Lab inference may produce belief semantics. See [`docs/SEMANTIC_DECISIONS.md`](docs/SEMANTIC_DECISIONS.md) and [`THIRD_PARTY.md`](THIRD_PARTY.md).

## Validate

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Validation runs the same format, Clippy, and unit-test commands through a pinned reusable workflow on pull requests and pushes to `main`.
