# Belief Lab

Provenance-first experiments for turning heterogeneous evidence into explainable beliefs.

## Safety-first policy boundary

The first implementation slice is `belief-policy`: a fail-closed authorization layer that decides which evidence classes, evidence purposes, and inference classes a process may use.

Configuration is supplied through environment variables. See [`.env.example`](.env.example) and [`docs/POLICY.md`](docs/POLICY.md).

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
