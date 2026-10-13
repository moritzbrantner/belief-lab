# Source-local comparison retrospective acceptance

Authority: `docs/DECISION_ATLAS.md` at `0ac73bf706cee654a8013356d2a27bd4a5680122`, Decisions 1.4.1, 1.4.3, 1.4.7 and 1.4.9; issue #37's bounded source-local comparison slice in PR #38. This is retrospective verification by a separate acceptance context after production commits already existed. It does not assert tests-first chronology or independent authorship of earlier tests.

The actual preimplementation baseline is `0ac73bf` (parent of first production commit `d082833`), not the parent of the later browser-validation commit. Baseline inspection found neither the person-attribution application API nor `site/people.html`. Serving that baseline's site and requesting `/people.html` returned HTTP 404. This is absent-surface evidence, not an executed semantic test result.

## Contract

- Compare distinct source-local people on the same scoped proposition through the real Rust/WebAssembly path. Display explicit, reported and inferred observations, original passages, source/producer revisions, authorization and assessment provenance.
- Keep observation confidence, person confidence and conviction separate. A clear tentative belief can have high person confidence and low self-reported conviction. Inferred conviction remains labeled separately. Neither score is objective truth or silently relabeled `soft_truth`.
- Quotations/criticisms cannot silently establish endorsement. An assessment lacking person confidence must be refused or remain unresolved rather than assert a definite personal belief.
- A reviewed contradictory observation remains contrary evidence, never supporting evidence for the opposite stance. Reviewing a conflict does not alter the source observation.
- Preserve the user's exact source passage, including leading/trailing whitespace, with its original offsets through recording, comparison and editable export.
- Same display names do not join identities. Bind people to source repository, scope and subject. Preserve policy refusals and prohibit score-semantics injection.

## Executable evidence

`apps/belief-cli/tests/person_belief.rs` (existing, independently inspected after deriving this contract) covers distinct scores and assessors, self-reported/inferred conviction, quote exclusion, unknown scores, conflict review, source binding and policy refusal. The six tests pass at `8aa9eca`.

`apps/belief-cli/tests/person_belief_acceptance.rs` adds independently authored retrospective cases for missing person confidence and contrary observations labeled as support. Both fail at `8aa9eca`: the former outputs `endorses`, and the latter includes `a-quotation` in supporting IDs despite its `disputes` stance.

`site/browser-test.mjs` already exercises the integrated Pages comparison and editable annotation export; the retrospective extension records a passage with surrounding whitespace and nonzero offsets, then checks both comparison and export. The advanced editor must be opened before Playwright fills its input; doing so preserves the original fixture and assertion semantics. AssessedScore's JSON assessor uses `assessedBy` while conviction's report uses `assessed_by`; rendering either preserves the established scripted/analyst provenance rather than changing product expectations.

With isolated `wasm-bindgen-cli 0.2.128` on PATH, `scripts/build-pages.sh` passes. Real headless Chromium at `8aa9eca` plus retrospective tests reaches the new whitespace assertion and fails because recording trims the source passage. Earlier checks in the same execution pass through scripted comparison, visible score provenance and recording a quotation; checks after the failing assertion are unrun. T3 preview status/open explicitly report no automation host, so the documented shell Playwright harness is used.

Focused command: `cargo test -p belief-cli --test person_belief --test person_belief_acceptance`. Frontend gate: `scripts/build-pages.sh`, then `node site/browser-test.mjs`. The complete Rust/Pages completion gates remain required: there is no complete proven affected-test dependency map.

## Limitations and owner reproduction

This is an illustrative, investigator-authored/scripted source-reference fixture, not producer-fetched source verification, real-model inference, calibrated probability, independent real-input product acceptance or live deployment evidence. The bounded slice does not deliver context suggestions, saved attribution history, persistent investigations or cross-source identity linking from broader issue #37.

Open `/people.html`, inspect the two people's opposing scoped positions and all three scores, inspect original passages/provenance, record a quotation with unknown scores, and download editable annotations. Public Pages must be verified separately against the merged revision. No public route or overall issue acceptance is claimed here.

Convention resolution sourceRevision: `46d8793bb3034326561f876dcc67dbaa5aa1e432`.
