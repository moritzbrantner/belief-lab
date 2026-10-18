# Attribution revision-history acceptance (#37)

Independent contract established against baseline
`866d4cfa8cabeab6089fabdfae38ef67026481c9` before production implementation.
Authority: issue #37's owner comment
https://github.com/moritzbrantner/belief-lab/issues/37#issuecomment-6186842196,
Atlas Decisions 1.3.2 as reapplied by 1.4.1, 1.4.3, 1.4.7, 1.4.9 and
1.4.25. Shared conventions were resolved in this context at sourceRevision
`6cc05789dab39cee6758b288f83fb48b4ee6dbdb`; repository AGENTS.md/CLAUDE.md apply.

This bounded slice keeps ordered investigator-supplied annotation revisions and
reasons, presents earlier person comparisons as historical, supports corrections,
withdrawals and explicit reassessment, and preserves editable revision-log export.
It does not deliver #37's entire semantic/context/current-stance/identity scope,
#35 portable investigations, or producer verification of annotated source bytes.
Revision order describes changes in the investigator's evidence/interpretation;
it is not proof that a person changed their mind. No expression/publication dates
or objective proposition-truth series are fabricated.

## Shared public contract

Tests use `belief_cli::attribution::compare(ComparisonInput)` and existing prepared
`examples/person-attributions/` evidence and annotations. V1 remains supported.
V2 has the same schema name, version 2 and an ordered nonempty `revisions` array;
each revision has a nonempty `reason` and full V1 annotation `document`. Historical
entries in the serialized report contain `revision`, `reason` and `people`; they
exclude the current revision, whose people stay at the top level. Current revision
metadata is separately available. JSON formatting does not change replay results.
This contract does not establish that an imported prior snapshot is an authentic
record of a past user action; the UI must label history investigator-supplied.

Every snapshot crosses the real existing evidence/policy/attribution validation
boundary. A valid latest snapshot cannot hide malformed or unauthorized historical
source bindings. Historical passages, spans, source-local people, provenance,
three score meanings and supporting observations remain inspectable. Correction
uses a new observation ID, with the old observation retained in its earlier
snapshot; withdrawal removes the record from the current snapshot only. Explicit
supersession metadata may identify the corrected record. Current assessments must
not silently survive changed or newly added evidence. Explicit reassessment may
record the same numeric score with a new review marker/reason; neither equality
of scores nor a generic revision reason proves that review occurred.

## Independent Rust checks

`apps/belief-cli/tests/attribution_history.rs` was authored before production and
covers:

| Check | Observable acceptance |
| --- | --- |
| V1 unchanged | Existing opposing source-local people and their person confidence remain intact; no fabricated historical revision |
| Ordered correction/withdrawal | Prior comparisons exactly match separately computed V1 snapshots, including passages/spans/provenance; current withdrawn observation absent; unaffected person unchanged |
| Stale assessment | New or changed observation with the old assessment cannot retain a definite person-confidence score; refusal or unknown accepted |
| Explicit reassessment | Same observations, newly assessed confidence, old/new confidence separately inspectable |
| Invalid history | Empty history/reasons, missing/nested document and invalid old evidence/person binding rejected |
| Replay | Serialized/deserialized editable V2 input produces identical comparison/history; no truth or soft-truth field |

Actual baseline command: `cargo test -p belief-cli --test attribution_history`.
Result: **3 passed, 2 failed**. Correction/withdrawal and reassessment fail because
baseline parser rejects `revisions` as an unknown field. V1 compatibility passes.
The negative/stale cases pass because V2 is rejected entirely; they do not yet
prove an implemented V2 validation or staleness boundary. No Playwright acceptance
was executed in this independent context at contract establishment.

## Real Playwright contract before production

Extend the existing real-WASM `site/browser-test.mjs` acceptance against
`scripts/build-pages.sh` output. Use actual accessible controls and downloaded
bytes, never mocked reports or a JS inference substitute. These scenarios are
established before implementation; concrete control names follow the implemented
accessible interface rather than an invented DOM API.

1. Open `people.html` with V1 prepared input. Inspect both source-local people,
   three score meanings and original passage. Make an annotation change with an
   investigator reason. See current revision and inspect the original historical
   comparison, visibly labeled not current and investigator-supplied.
2. Add a quotation/new observation. Current affected person confidence becomes
   unknown; earlier confidence remains historical. The other person's observations
   and assessment remain unchanged. A reason alone cannot silently reapprove the
   old assessment. Preserve exact whitespace and supplied offsets.
3. Correct an observation with a new ID, explicit supersession and reason. Inspect
   the new current passage and the old historical passage/spans/provenance. Neither
   becomes a claim of changed mind or objective truth. Withdraw the corrected
   observation with reason; it disappears only from current contributions.
4. Explicitly assess/reassess a person with reason, including a review retaining
   the same score after changed evidence. Verify the explicit review action,
   current score and earlier unknown/scored snapshots remain distinguishable.
5. Download editable annotations through the real browser download event, import
   those exact bytes in a clean page, inspect current and historical comparisons,
   continue with another revision, then download/reimport again. The ordered log,
   reasons, exact source text and provenance survive; this is annotation-history
   continuity, not complete portable investigation delivery.
6. Import malformed history, whitespace-only reason and invalid historical source
   binding. Show actionable refusal and retain the existing editable input/current
   valid state; no unauthorized historical snapshot becomes displayable as valid.
   Attempt a mutation with an empty reason: it must not silently append a revision.
7. Inspect current/history at desktop and 390 px mobile widths, select an older
   revision and return to current; historical controls must not overwrite current
   input. No browser errors or horizontal overflow. Capture actual history and
   correction views under ignored `target/` for owner review.

History is computed by Rust for every snapshot. UI annotations and review actions
are not independent inference. Passing screenshot capture does not prove semantic
correctness, deployment status, or owner acceptance.

## Evidence to close this slice

Record final Rust/browser test paths and results at their actual revision. Run
required format, Clippy, workspace tests and offline demo plus Pages build and
`npm --prefix site test`. Compare any changed acceptance expectations against the
approved slice, preserving this baseline evidence. Public postdeployment checking
and owner acceptance remain separate from local passing tests. Keep #37 open for
its remaining approved scope.
