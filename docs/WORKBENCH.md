# Evidence workbench

The [GitHub Pages workbench](https://moritzbrantner.github.io/belief-lab/) loads downloadable evidence and judgment JSON, runs the actual Rust explanation pipeline through WebAssembly, and shows admission decisions, beliefs, correlated judgments, and provenance. Files are processed in the browser and are not uploaded. The example selector supports shareable `?example=correlated-evidence` URLs.

Examples are deliberately scripted: their scores are not live model predictions. The local `workbench` and `decide` commands execute real models; Pages offers its request file and accepts the saved result for inspection. Raw audio/video extraction remains the producer repositories' responsibility. Upload evidence-interchange JSON, not arbitrary media.

## Run models interactively

```sh
cargo run -- workbench
```

Open the complete URL printed by the command. The request is preloaded; select a model and click **Run analysis**. You can edit the request, upload a downloaded request JSON file, inspect scores and source provenance, and download the result. The interface supports English, German, Spanish, light/dark themes, and mobile layouts.

Assets are embedded in the native executable: no Node, WebAssembly build, or separate web server is needed. Startup does not acquire models. The first authorized model request prepares the pinned SemIf environment and chosen model, with progress in the terminal. Subsequent requests reuse those resources. Select the explicitly scripted provider to exercise the same pipeline without models or network access.

The server binds only to `127.0.0.1` on an available port. Its session URL carries a random capability in the fragment; POST requests require that token and the exact local origin. The server serves only embedded assets, bounds uploads to 1 MiB and 15 seconds, and permits one analysis at a time. Closing the tab does not cancel an in-flight model operation; stop the command with Ctrl+C. The local adapter invokes the same validation, authorization, provider resolution, and execution function as `decide`; it is not a remotely hosted inference service.

## Run a model on a file

From a fresh clone:

```sh
cargo run -- decide examples/decisions/preference.json > result.json
```

This authorizes the request before installing or invoking SemIf. First use downloads the pinned provider and phone-tier model. Later calls reuse them. Progress goes to stderr; stdout contains exactly one JSON result. Open `result.json` in the workbench to inspect it. Change `provider.tier` to `desktop` or `high-memory` to select another pinned model. `examples/decisions/fixture.json` runs the same boundary entirely offline with explicitly simulated scores.

## Build and verify Pages locally

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
scripts/build-pages.sh
python3 -m http.server --directory target/pages 8765
```

Open http://localhost:8765. Assets and downloadable examples are assembled under `target/pages`; no generated bundle is committed. Node is only needed for browser acceptance tests:

```sh
npm --prefix site ci
(cd site && npx playwright install chromium)
npm --prefix site test
```

The local browser acceptance test uses the scripted provider and starts its own native server:

```sh
npm --prefix site run test:local
# Explicit real-model browser acceptance (prepares missing resources):
npm --prefix site run test:models
```

The Pages workflow builds and tests pull requests and deploys `main` after its browser checks pass. Rust workspace checks continue independently of browser tools and models.

## Roadmap coverage

- #11: declared ownership in `repository-purpose.json`, strict reference-only imports, offline core, checksum-verified setup, and an intentional Pages workbench. The corpus fixture is illustrative; producer-export compatibility remains conditional on real producer exports becoming available.
- #13: exact-target model panels, deterministic descriptive summaries, source-family correlation through `panel_bases`, and preservation of all members in store explanations. Panels do not schedule providers or turn votes into probabilities.
- #14: versioned one-decision JSON/stdio command with validation and authorization before provider resolution, stable errors, exact provenance, and offline fixture-provider tests.

Verification convention source revision: `e6acb5310afaf15c0cba24f87108f5f4ad1bedc3`. This records the policy used for this implementation; it does not pin future policy consumption.

Real-provider acceptance (explicitly downloads any missing resources, then executes all three CPU model tiers):

```sh
cargo test -p belief-cli --test real_models -- --ignored --nocapture
```

After setup, repeat with `HF_HUB_OFFLINE=1 PIP_NO_INDEX=1` to verify reuse of the prepared resources. The ordinary workspace test command skips this acceptance test.

## Person-belief comparison on Pages

Open `people.html` from the Pages workbench. The prepared `examples/person-attributions/` fixture contains a single scoped proposition, two distinct source-local people, explicit, reported, inferred, and quotation-only observations, exact investigator-supplied passages with offsets, and a three-score separation example. The people disagree, but the report contains **no truth verdict**.

The browser form records a new observation from an already imported evidence reference; it requires a source-local subject, scope, passage and offsets. Confidence (observation) and confidence (person) are separately assessed, conviction is self-reported or inferred, and empty values stay unknown. Recording a new observation invalidates the person's previous overall assessment instead of trusting it on changed evidence. Provenance shows source/producer revisions and the admitted evidence purpose. No JavaScript inference is performed.

This is a prepared-source attribution surface, not a live producer integration or provenance verification service. The excerpt text/offsets are annotations supplied by the user, not independently matched to upstream bytes. The scripted fixture scores are demonstrative, not empirical or calibrated. The old `soft_truth` workbench remains a distinct historical/reference view. Complete investigation storage, comparison-context review and identity linking are not implemented here. Annotation correction, withdrawal and reassessment history are supported as described below.

Run `npm --prefix site test` after `scripts/build-pages.sh` for a real-browser comparison, analyst-entry, score-separation and rejection regression. The test captures `target/person-belief-comparison.png` for owner acceptance.

Person annotations bind to the tuple `(sourceRepository, scopeId, subjectId)`. Identical names or subject tokens from different repositories never implicitly unify; the browser creates separate view records for collisions.

After recording, **Download editable annotations** saves the source-linked annotation JSON (including revisions and manually added observations) for later reimport; **Download comparison** saves the validated Rust report. No account or server-side investigation storage is implied.


## Attribution revision history

On `people.html`, provide a **Revision reason** before recording, correcting or withdrawing
an observation, or reassessing a person. **Correct <observation ID>** loads that annotation
into the form; recording creates a new ID with an explicit supersession link. Withdrawal
removes the observation from the current comparison while retaining its earlier passage,
offsets, provenance and assessment in history. These actions invalidate the affected person's
previous confidence. Other source-local people keep their existing assessments.

Use **Reassess person and compare** to record an explicit new assessment with selected
supporting observations, confidence and conflict review. Unknown values stay unknown.
Each current and historical card identifies which observations contribute to its assessment.
An unchanged numerical score can be recorded as a fresh explicit review; a reason by itself
cannot refresh a stale carried-forward assessment.

**Attribution history** opens earlier snapshots, labeled investigator-supplied and not current.
Revision order records changes in annotations or interpretation, not the dates of the person's
expressions or proof of a change of mind. No expression dates, timestamps, truth scores or
authenticated audit history are invented. The original reason for imported version-1 annotations
is labeled unknown rather than attributed to an investigator.

**Download editable annotations** now preserves the ordered revision log. Reimport that file
with the original evidence references, inspect its history, add another revision, and export
again. Invalid file imports leave the current editable annotations and comparison intact.
Manual edits in the advanced editor remain uncommitted input until comparison succeeds.
This transfers annotation history only: complete portable investigations, source recovery,
workspace identity links and cross-investigation comparisons remain in #35–#37.

The version-1 annotation schema remains accepted with its existing report. Version 2 uses
`{"schema":"belief_person_attributions","schemaVersion":2,"revisions":[...]}`.
Each revision has a nonempty `reason` and a full version-1 `document`. Optional
`reassessedPersonIds` marks an explicit review retaining unchanged assessment values;
optional `supersedes` maps each new observation ID to the prior ID it corrects.
Reports contain current `revision`/`revision_reason`, optional current `supersedes`, and
historical `history` entries with revision, reason, people and supersession links.

The Rust comparison validates every snapshot under the supplied evidence and **current**
policy, importing that evidence once per comparison. Earlier snapshots are not independent
historical authorization receipts. Keep the relevant historical evidence references available;
an invalid old snapshot cannot be hidden behind a valid latest one. History requires 1–100
revisions, one unchanged scoped proposition, stable source-person identities and immutable
observation IDs. Corrections need new IDs. Imported history is investigator-supplied and may
have been edited outside Belief Lab; validation does not authenticate its origin or verify
original producer bytes.

Independent preimplementation scenarios and recorded baseline results are in
`docs/acceptance/attribution-history.md`. Rust acceptance runs in the workspace suite; the
real-WASM browser suite covers correction, withdrawal, reassessment, invalid imports and two
round trips of downloaded history, and captures history/correction views under `target/`.
