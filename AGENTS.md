# Agent Instructions

Provenance-first experiments for turning heterogeneous evidence into explainable beliefs.
Pure Rust workspace; `cargo run` executes a deterministic offline demo of the full evidence → judgment → policy → inference → store → explanation path.

## Read first

- `README.md`, then the doc for the area you change: `docs/ARCHITECTURE.md`, `docs/POLICY.md`, `docs/EVIDENCE_INTERCHANGE.md`, `docs/SEMANTIC_DECISIONS.md`, `docs/INFERENCE.md`, `docs/STORE.md`.

## Layout

| Crate | Role |
| --- | --- |
| `apps/belief-cli` | Top-level CLI (`cargo run`, `doctor`, `setup`, `semantic-demo`) |
| `belief-core` | Evidence, judgments, claims, beliefs, derivations, provenance, score semantics |
| `belief-policy` | Fail-closed authorization over evidence and inference classes |
| `evidence-interchange` | Versioned, policy-admitted references to producer-owned evidence |
| `semantic-decision` | Provider-neutral, policy-gated semantic judgments with receipts |
| `semif-provider` | Pinned local SemIf adapter plus source/model bootstrap |
| `inference-core` | Authorization boundary for inference execution |
| `inference-baseline` | Deterministic, correlation-aware soft-truth reference inference |
| `belief-store` | Explanation, import receipts, transitive invalidation |

## Commands

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --quiet        # offline demo; CI runs it, must stay green without network or models
```

`cargo run -- setup` / `semantic-demo` download SemIf and a ~639 MB model into `.local/`. Do not run them unless the task needs real semantic scoring.

## Invariants

- Policy is fail-closed. Sensitive-trait inference and real-world identity resolution are not authorized by any shipped profile and must not become enableable through `.env` alone.
- belief-lab does not own detector, ASR, OCR, NER or scene truth; it consumes provenance-preserving references from producer repositories.
- SemIf option scores are conditional option probabilities. Only belief-lab inference produces belief semantics; never relabel one as the other.
- Every judgment keeps provider/model/prompt provenance. SemIf and model revisions stay pinned; a dirty or mismatched environment is refused, not silently accepted.
- The default path (`cargo run`, tests) stays deterministic and offline.

## Git and merging

- Work on a branch named `agent/<short-topic>`; never commit directly to `main`.
- Open a PR, wait for CI, and merge it yourself with a merge commit (`gh pr merge --merge --delete-branch`) when all checks are green.
- You may also review and merge Renovate PRs, other agents' PRs and the owner's feature PRs once they are reviewed and green.
- Never weaken, skip, or delete a failing test or check to get green.

## Design decisions

- Implement directly; no planning issue is needed first.
- When you make a real architecture decision (new boundary, dependency, persistence/protocol shape, trade-off that is hard to reverse), record it as an ADR in `docs/adr/NNNN-<slug>.md` in the same PR.

## Shared foundations

- Evidence producers live beside this repo under `~/privat/`: `youtube-corpus`, `audio-analysis`, `visual-analysis`, `nlp-stack`. If a task needs a producer-side change, change it there: PR, merge when green, then bump the pinned rev here in the same task. Do not work around a producer bug locally or take over producer truth.

## Testing

- Every behavior change or bug fix comes with a test. For bugs, write the failing test that reproduces it first, then fix.

## Done means

- Format, Clippy, `cargo test --workspace` and `cargo run --quiet` pass.
- The relevant `docs/*.md` is updated when semantics or boundaries change.
