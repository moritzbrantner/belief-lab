# Belief Lab

Provenance-first experiments for turning heterogeneous evidence into explainable beliefs.

## Architecture

The current foundation separates:

- `belief-core` — evidence, judgments, claims, beliefs, derivations, provenance, and score semantics;
- `belief-policy` — fail-closed authorization over evidence and inference classes;
- `evidence-interchange` — versioned, policy-admitted references to producer-owned evidence;
- `inference-core` — the authorization boundary for inference execution;
- `inference-baseline` — deterministic, correlation-aware soft-truth reference inference;
- `belief-store` — explanation, import receipts, and transitive invalidation semantics.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), [`docs/POLICY.md`](docs/POLICY.md), [`docs/EVIDENCE_INTERCHANGE.md`](docs/EVIDENCE_INTERCHANGE.md), [`docs/INFERENCE.md`](docs/INFERENCE.md), and [`docs/STORE.md`](docs/STORE.md).

## Safety-first policy boundary

Configuration is supplied through environment variables. See [`.env.example`](.env.example).

The shipped profiles are deliberately bounded:

- `observe_only` — no inference.
- `semantic_research` — non-biometric semantic evidence and low-risk inference.
- `multimodal_research` — may use producer-owned face/voice track references, corpus-local identity links, and cross-source association only when each is separately enabled.

Sensitive-trait inference and real-world identity resolution are not authorized by any shipped profile. They cannot be enabled by changing `.env` alone.

The policy layer does not own detector, ASR, OCR, NER, or scene truth. Those remain in their producer repositories; `belief-lab` consumes provenance-preserving references downstream.

## Validate

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Validation runs the same format, Clippy, and unit-test commands through a pinned reusable workflow on pull requests and pushes to `main`.
