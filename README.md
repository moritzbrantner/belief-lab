# Belief Lab

Source-grounded investigation of **which people express or hold which beliefs**, without deciding whether the propositions are objectively true. The legacy proposition-level baseline remains available as a separately labeled heuristic, not as an attribution score.

## Compare attributed beliefs

[Open the person-belief comparison](https://moritzbrantner.github.io/belief-lab/people.html) to examine who endorses P, endorses not-P, explicitly suspends judgment or has no stance evidence for one scoped proposition, including conflicting and dated statements beside an explicitly uncertain current stance. Several differently worded propositions can be related by reviewable equivalence, one-way implication or partial-overlap suggestions, which never attribute the related proposition to anyone. Investigations are saved in the browser (or on disk by the local workbench) and move between devices as portable project files. The example distinguishes **confidence (observation)**, **confidence (person)**, and **conviction**, shows exact *supplied* source passages plus producer revisions, and leaves unsupported values unknown. You can record a new analyst annotation from an imported producer evidence reference.

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

### Original-source golden test (Russell–Copleston, #63)

`fixtures/golden/russell-copleston-1948/` holds the owner-approved expectations and the committed original audio stream. `audio/source.json` records where the audio came from and its digest. `cargo test --workspace` checks those bytes and the scoring code offline. The real-model run is opt-in. It runs real producer code from audio-analysis: Whisper medium (native Candle) and pyannote community-1 (native ONNX). It then scores golden-excerpt recall in the transcript and speaker-turn agreement (overall and per speaker), and writes a per-stage report to `target/golden/russell-copleston-1948/report.json`:

**One command** on your workstation (Python 3 standard library; no Python environment to install):

```bash
python3 scripts/russell_copleston_stage4.py
```

To run Whisper medium on an NVIDIA GPU (recommended for the first 17-minute audio pass),
use a working CUDA toolkit and NVIDIA driver, then run:

```bash
nvidia-smi
nvcc --version
python3 scripts/russell_copleston_stage4.py --gpu
```

`--gpu` compiles the pinned `audio-analysis` dependency with its `cuda` feature, then
requests CUDA device 0 explicitly. Whisper uses FP16 on CUDA instead of CPU FP32.
It fails if CUDA cannot initialize instead of silently running Whisper on CPU.
The pyannote community-1 ONNX stage still uses its existing runtime; this flag
does not imply GPU acceleration for that stage or subsequent e5/NLI scoring.
The first CUDA build may take several minutes. Later runs reuse the compiled
artifact and cached stage outputs. To run only Stages 1–3 with CUDA:

```bash
cargo test --release --manifest-path crates/golden-media/Cargo.toml \
  --features cuda --test russell_copleston -- --ignored --nocapture
```

The runner validates the committed audio and each saved stage's source/model receipts.
It reuses transcript/turns, pinned proposition-api candidate drafts, and the recorded
e5/NLI score if the offline replay matches the saved baseline. Missing stages run in order,
never concurrently. It fetches proposition-api at its **pinned commit** into
`target/golden/russell-copleston-1948/.producers/` when required, starts its pinned local
`llama-server`, and stops only that server when extraction ends. Interrupted Qwen draft windows
resume from their request-hash cache. Re-running with complete inputs performs only the
cheap offline replay, and unchanged diagnostic files are not rewritten.

To **require offline-only behavior** (no downloads or model execution), run:

```bash
python3 scripts/russell_copleston_stage4.py --replay-only
```

A completely fresh checkout contains the original audio but **not** the model weights or
intermediate inference results. The first full run needs Rust/Cargo, Bun, `llama-server` and
the pinned local model bundles from their owning producers. In particular,
`GOLDEN_PYANNOTE_BUNDLE` must point to the approved gated community-1 ONNX snapshot, and
`ORT_DYLIB_PATH` must resolve the compatible ONNX Runtime library if it is not installed in
the default search path. The Qwen3 model can be downloaded using the exact command printed
by proposition-api's `scripts/llama-server.sh --env`; the e5/NLI bundles must be materialized
through nlp-stack/model-runtime. The runner fails with the missing prerequisite instead of
substituting heuristics or pretending the acceptance test passed. A new machine without those
gated assets cannot finish solely from a Git checkout. Once they are prepared, the command
is resumable and does not redownload or re-infer on subsequent runs.

When Stage 4 produces its current known golden failures, the runner still preserves the
complete report and replays its score before reporting diagnostics. It prints the failed
acceptance warning separately from a model/runtime failure.

The individual stages are still available for troubleshooting:

```bash
M=crates/golden-media/Cargo.toml
# Stages 1-3: Whisper medium + pyannote. Persists transcript.json, speaker-turns.json and
# stage4-document.json, so stage 4 re-runs without re-transcribing.
ORT_DYLIB_PATH=/path/to/libonnxruntime.so GOLDEN_PYANNOTE_BUNDLE=/path/to/approved/community-1-onnx-snapshot \
  cargo test --release --manifest-path $M --test russell_copleston -- --ignored --nocapture

# Stage 4a: draft claims with proposition-api (main at the commit pinned in stage4/extract.ts)
# against its pinned Qwen3-8B llama-server; writes stage4-candidates.json.
(cd ../proposition-api && scripts/llama-server.sh) &
eval "$(../proposition-api/scripts/llama-server.sh --env)"
bun crates/golden-media/stage4/extract.ts ../proposition-api target/golden/russell-copleston-1948

# Stage 4b: match claims with multilingual-e5-base (nlp-stack#188 preset, cached bundle), map
# attributions, score, and export them to belief-lab's evidence and attribution contracts.
cargo test --release --manifest-path $M --test russell_copleston_stage4 -- --ignored --nocapture

# Replay the saved Stage 4 inputs, including similarities.json and polarity.json, entirely
# offline. No Whisper, pyannote, Qwen, embeddings or NLI model is loaded. The command refuses
# source-document digest mismatches and scoring drift from attributions-report.json.
cargo run -p golden-scoring --bin stage4-diagnostics -- \
  fixtures/golden/russell-copleston-1948/expected-beliefs.json \
  target/golden/russell-copleston-1948
```

`crates/golden-media` sits outside the workspace, so ordinary builds never compile the model stacks. Whisper medium downloads on first use. `GOLDEN_THREADS` caps CPU threads (default 4); `--gpu` accelerates Whisper and leaves other stage execution unchanged. The approved pyannote snapshot is the one audio-analysis's `loadCommunityDiarizationResources` materializes; its two gated PLDA inputs can come from a local Hugging Face cache of the pinned revision (the loader checks every digest).

Stage 4 follows the owner decisions of 2026-11-04 (#63):

- **Document.** The transcript text, one segment per speaker turn (consecutive Whisper segments whose longest-overlapping diarization label maps to the same golden person). The label → person mapping is the one-to-one assignment with the most agreement against the golden turns, as in the stage-3 score. A byte offset's time is interpolated linearly inside its Whisper segment.
- **Spans.** A candidate is attributed only when its quote is exactly the transcript bytes at its offsets and its speaker is its turn's speaker.
- **Stance.** For the speaker (empty reporting) or the innermost holder: `asserted` → `endorses`, `negated` → `endorses_negation`, `questioned` → `no_stance`, `hedged` → `no_stance` plus a `hedged` flag (never `suspends_judgment`), `conditional` (only supposed; proposition-api draft-v3) → `no_stance`. Outer reporting layers whose holder is the speaker ("I think") are the speaker's own assertion. Otherwise every outer reporter gets `no_stance` / `reported_position` with `aboutPerson` and `attributedStance`, and a holder who is a golden person gets the stance as a `reported_position` `via` the reporter. Holders resolve to golden people by a unique name word ("Father Copleston" → `copleston`); others ("Sartre", "he") stay unresolved.
- **Matching.** A claim matches an expected attribution when the e5 cosine (symmetric input) between its canonical text and the golden proposition is at least `SIMILARITY_THRESHOLD` and its time is within ±30 s of `approxSeconds`. **Polarity** is flip-only (owner decision 2026-11-04, revised after #85). The pinned multilingual NLI preset (`MoritzLaurer/mDeBERTa-v3-base-xnli-multilingual-nli-2mil7` @ `b5113eb`, nlp-stack#195) judges the claim's canonical text (premise) against the golden proposition (hypothesis). It only flips the stance (`endorses` ↔ `endorses_negation`) when the contradiction probability is at least `MIN_CONTRADICTION`. Neutral and entailment keep the cosine match and the stance. The judgments with their probabilities are written to `polarity.json`, and a pair at or above the threshold without one fails the run. An attribution is a hit when person, stance, kind, reporter (`via`/`aboutPerson`/`attributedStance`) and span (half the words of the shorter quote shared) all agree. `no_stance_evidence` entries and `expectedNoAttribution` rules are violated by an attributed stance on a matching claim; for the latter only from the person's own words (a report `via` someone is allowed). A fabricated attribution states a stance on a matched golden proposition that the golden file does not give that person.
- **Report.** `attributions-report.json` (every expected entry with its best check, fabrications, near-misses within 0.05 below the threshold or 30–60 s away, and the threshold calibration), `attributions-summary.md`, `claims.json`, and `belief-lab-evidence.json` / `belief-lab-investigation.json` (a `belief_evidence_interchange` v2 batch and a `belief_person_attributions` v3 investigation that belief-lab's attribution report accepts; the reporter chain is evidence parentage, the `hedged` flag has no field there). `attributions-diagnostics.json` and `.md` cover every expectation, fabrication, and forbidden own-words attribution with the candidate's exact quote/span, speaker/turn, stance cue, reporting chain, similarity, NLI label/probabilities, and observed failed gates. Candidate views include nearby times, source-quote overlaps and high-cosine alternatives; observed gate failures are not asserted to be model-error root causes. The offline replay command regenerates only these diagnostic files and checks the exact saved scoring baseline first.

#### Stage 4 calibration

`SIMILARITY_THRESHOLD` is **0.91** and `MIN_CONTRADICTION` is **0.90**. Both come from the flip-only sweep (`calibration.thresholdSweep` in `attributions-report.json`, cosine 0.80–0.95 × contradiction 0.70–0.95) on the existing draft-v3 run. That run used:

- Whisper medium + pyannote community-1;
- proposition-api `d3613a0`, draft-v3: 149 grounded candidates;
- multilingual-e5-base;
- mDeBERTa NLI on the 603 pairs with cosine ≥ 0.80.

| cosine | contradiction ≥ | hit | partial | miss | fabricated | `no_stance_evidence` violated |
|---|---|---|---|---|---|---|
| 0.80 | 0.70–0.85 | 4 | 9 | 0 | 192–195 | 1 |
| 0.80 | 0.90 | 5 | 8 | 0 | 191 | 1 |
| 0.87 | 0.70–0.85 | 4 | 6 | 3 | 23 | 1 |
| 0.87 | 0.90 | 5 | 5 | 3 | 22 | 1 |
| 0.88 | 0.90 | 4 | 1 | 8 | 9 | 1 |
| 0.89 | any | 3 | 1 | 9 | 6 | 0 |
| 0.90 | any | 3 | 1 | 9 | 3 | 0 |
| **0.91** | **0.90** | **3** | **0** | **10** | **2** | **0** |
| 0.93 | any | 2 | 0 | 11 | 2 | 0 |
| 0.95 | any | 0 | 0 | 13 | 1 | 0 |

How the pair was picked:

- **Contradiction 0.90:** below 0.90, NLI flips a correct match (one hit is lost at every threshold), and above it nothing changes.
- **Cosine 0.91:** the lowest threshold at which fabrications reach their floor of 2 and `no_stance_evidence` holds. Fabricated attributions count against acceptance, so 0.87 (5 hits, 22 fabricated) was not chosen.

`expectedNoAttribution` (russell / god-exists) is violated at every point.

Cosine-only (`thresholdSweepCosineOnly`) gives the same 3 hits and 2 fabricated at 0.91, so on this run flip-only NLI at 0.90 changes the outcome only at lower thresholds (one fewer fabrication). The earlier "neutral is no match" rule (#85, threshold 0.92) gave 2 hits. e5 alone does not separate the abstract golden propositions: they score 0.749–0.904 against each other.

Known gaps, recorded rather than worked around:

- **Kinds the mapping does not produce:** `inferred_belief` (#13) and `quotation_or_criticism` (#14).
- **Hypotheticals:** draft-v3 marks Russell's "Then God exists" (537 s) as `asserted`, which violates `expectedNoAttribution`. proposition-api draft-v4 adds the preceding turns as read-only context for this; it has not been re-drafted yet (see #63).
- **Name spelling:** Whisper writes "Coppiston", and the narrator's reports (#1–#3) are missed.

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
