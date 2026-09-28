# Agent Instructions

Provenance-first experiments for turning heterogeneous evidence into explainable beliefs.
Pure Rust workspace; `cargo run` executes a deterministic offline demo of the full evidence → judgment → policy → inference → store → explanation path.

## Read first

- `README.md`, then the doc for the area you change: `docs/ARCHITECTURE.md`, `docs/POLICY.md`, `docs/EVIDENCE_INTERCHANGE.md`, `docs/SEMANTIC_DECISIONS.md`, `docs/INFERENCE.md`, `docs/STORE.md`.

## Layout

| Crate | Role |
| --- | --- |
| `apps/belief-cli` | Top-level CLI (`cargo run`, `explain`, `doctor`, `setup`, `semantic-demo`) |
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

# See what the pipeline does for an input instead of reading the code (offline, deterministic):
cargo run -q -- explain <evidence.json> [--judgments <script.json>] [--profile <name>] \
  [--policy BELIEF_KEY=VALUE]... [--json]
# e.g. cargo run -q -- explain fixtures/explain/semantic-research/evidence.json \
#        --judgments fixtures/explain/semantic-research/judgments.json --profile semantic_research

# Golden outputs for fixtures/explain/<case>/ run in `cargo test --workspace`; after an intended change:
BELIEF_UPDATE_FIXTURES=1 cargo test -p belief-cli --test explain_fixtures   # then review the diff
```

`explain` exits 0 when it produced an explanation (policy refusals included), 2 when an input was malformed or unsupported (fail-closed), and 1 on usage or I/O errors. It reads no policy from the environment; pass `--profile` and `--policy` explicitly. Add a case directory under `fixtures/explain/` for new behavior (see `docs/GETTING_STARTED.md`).

`cargo run -- setup` / `semantic-demo` download SemIf and a ~639 MB model into `.local/`. Run them when a task needs real semantic scoring; reuse an existing `.local/`.

## Invariants

- Policy is fail-closed. Sensitive-trait inference and real-world identity resolution are not authorized by any shipped profile and must not become enableable through `.env` alone.
- belief-lab does not own detector, ASR, OCR, NER or scene truth; it consumes provenance-preserving references from producer repositories.
- SemIf option scores are conditional option probabilities. Only belief-lab inference produces belief semantics; never relabel one as the other.
- Every judgment keeps provider/model/prompt provenance. SemIf and model revisions stay pinned; a dirty or mismatched environment is refused, not silently accepted.
- The default path (`cargo run`, tests) stays deterministic and offline.

## Shared conventions

General engineering rules (git and merging, commits, testing, ADRs, docs, dependencies, Rust style, …) come from `coding-agent-conventions`, installed in `.conventions/`. Read the rule briefing in `.conventions/index.md` before implementing and open the linked source when a rule applies. Do not edit `.conventions/`; refresh it with `coding-tooling conventions update`. Rules below are repository-specific additions or exceptions.

## Evidence producers

- Producers are checked out beside this repo under `~/privat/`: `youtube-corpus`, `audio-analysis`, `visual-analysis`, `nlp-stack`. Fix producer-side defects there and update the reference here (DEP-003); never take over producer truth.

## Done means

- Format, Clippy, `cargo test --workspace` and `cargo run --quiet` pass.
- The relevant `docs/*.md` is updated when semantics or boundaries change.
