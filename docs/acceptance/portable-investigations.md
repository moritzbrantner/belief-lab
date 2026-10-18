# Portable investigations acceptance contract (#35)

Status: independent preimplementation contract, not acceptance of an implementation.
Baseline: `fb317e61055b30e8d86e3f2cef165970f9379c2f`.
Convention resolver sourceRevision: `6cc05789dab39cee6758b288f83fb48b4ee6dbdb`.
Authority: issue #35 (body inspected; no comments), repository AGENTS.md and
CLAUDE.md, and Decision Atlas 1.1.2–1.1.10, 1.4.1, 1.4.3, 1.4.7,
1.4.9, 1.4.11–1.4.21, 1.4.29, 1.4.31 and 1.4.33.

This context inspected the baseline interfaces and test source independently of
production changes. No test, browser journey, deployment check or owner acceptance
was executed for this document. Scenarios below are required executable checks,
not passing results. No proposed Rust type, serialization field, function name,
control label or storage technology is prescribed here.

## Baseline and seams

The shared application exposes `attribution::compare(ComparisonInput)` and the
WASM `compare_attributions_json(evidence, attributions, policy)` wrapper. The
legacy `explain_json` runs proposition-level inference; its soft-truth output is
not a person-belief score. The current person annotation document has one scoped
proposition, source-local people, observations and assessments. Neither that
annotation file nor a rendered comparison is a portable investigation.

`site/browser-test.mjs` runs Chromium against the built Pages/WASM application;
it exercises person comparison, exact passage/offset retention, editable
annotation download, score separation and invalid input. `site/app.js` persists
language/theme, not investigations. `docs/WORKBENCH.md` explicitly identifies
investigation storage and identity linking as unimplemented. No existing portable
project, shared person registry, semantic-link or revision-adoption public seam
was found at this baseline. Tests must exercise the actual shared Rust project
boundary once implemented, and actual file/UI journeys in the real built browser;
passing annotation/report download tests cannot substitute for these scenarios.

Owner steering during this session resolves the policy-placement question for this
contract: non-biometric source-person linking belongs in the standard investigation
workflow, with explicit confirmation of each new match. This is session-authorized
intent; no new durable Atlas Decision is claimed here. The separately reviewed
policy prerequisite must implement that bounded authorization before the linking
journeys can pass. Test that existing confirmed links can be reused without a
second approval, while a new candidate requires explicit confirmation. Automatic
real-world identity inference, biometric matching and sensitive-trait inference
remain blocked; no environment switch may enable them.

## Deterministic fixture inventory

Construct small fixtures through the eventual public project boundary, retaining
source references rather than inventing producer formats. Existing
`examples/person-attributions/` evidence and annotations supply the prepared,
explicitly scripted starting point; they are not verified upstream source bytes.

- Investigation A, revision A1: source-grounded P and distinct scoped Q; two
  source-local people with colliding display names but distinct repository/scope/
  subject tuples. Include explicit, reported and inferred attributions, a quotation
  without endorsement, exact passages with whitespace/offsets, unknown scores,
  separately supported observation/person confidence, and self-reported and
  inferred conviction with provenance. Add dated endorsement, negation, explicit
  suspension and a person/proposition pair with no stance evidence. Include opposing
  same-context observations yielding a conflicted assessment and a review revision.
- A2: add independent evidence and a correlated derivative of an existing source;
  retain A1 explanations, import receipts, authorization input snapshots and a
  revoked source with its affected derivations. Available permitted source bytes S1
  have a declared immutable reference/revision and integrity binding. S2 is missing;
  S3 is available but not permitted to bundle. A full export must disclose both gaps.
- Investigation B: independent evidence and assessments, containing some of A's
  source-person records but no A observations. A workspace-only unrelated person
  link and investigation C act as export-leak sentinels.
- Link history L1: human-confirmed non-biometric source-record association relevant
  to A/B, original tuples, supporting evidence and approval/rejection/unlink history.
  L2 corrects/revokes L1. A adopts L2; B retains applied L1 pending correction.
- Conflict destination: an existing confirmed association contradicting an imported
  confirmed link, with different non-biometric supporting evidence and approval
  history. Include unrelated usable evidence in that imported investigation.
- Semantic history: separate source-grounded propositions and equivalence,
  directional P → Q, and partial-overlap suggestions, including accepted, rejected,
  corrected and unresolved revisions with evidence and uncertainty. There is no
  person-Q attribution justified solely by P → Q.
- Mutations of an otherwise valid export: malformed JSON/container, unknown version,
  unknown required representation, duplicate IDs with changed values, absent/cyclic
  dependencies, changed source bytes or provenance under an existing identity,
  altered score semantics, and fabricated/altered authorization or approval history.
  Match the final format's declared integrity/validation guarantees; a checksum
  alone does not establish trusted source provenance or human approval.

## Requirement-to-check mapping

Each R scenario needs a deterministic Rust integration/round-trip assertion across
the shared application boundary. Each B scenario needs Chromium against the
production-like Pages build and actual downloads/uploads. Assert semantic state
and visible disclosures, not just successful parsing or a screenshot.

| Scope and authority | Rust check | Real-browser check and expected outcome |
| --- | --- | --- |
| Isolation/persistence, 1.1.4/1.1.6 | R1: save A/B, reopen, add A2, compare B unchanged; repeat export/import | B1: create A and B, close/reopen browser session, resume A, switch to B; source lists, observations and history stay separate |
| Full portable history, 1.1.6 | R2: round-trip A1/A2 including receipts, selected/ignored correlated evidence, validity, exact source/producer/model/prompt provenance and ordered revisions | B2: import and inspect saved earlier and latest explanations, revoke/add evidence and inspect both; no report-only import |
| Attribution meanings, 1.4.1/.3/.7/.9/.29/.31 | R3: preserve each attribution kind, all stance distinctions, unknowns, three score dimensions, conviction basis, dated opposing observations and review history; no truth-score conversion | B3: inspect original passages, times and scores after transfer; absence stays absence, conflict remains conflicted rather than suspension or a fabricated current stance |
| Export choices, 1.1.8/1.4.17 | R4: all four matrix cases below; assert exact included/omitted materials and links, with sentinel exclusion | B4: independently choose both options, download, transfer to fresh context, import offline; disclosed coverage matches actual bytes and link availability |
| Shared links without shared beliefs, 1.4.11/.13/.15 | R5: confirm permitted L1 in A, reuse existing linked records in B without new approval; unmatched record stays unconfirmed | B5: inspect reused approval and original tuples in B; A's evidence/assessments never appear in B; new candidate needs explicit review |
| Missing originals and links, 1.1.10/1.4.17 | R6: open absent S2 and unavailable link references; historical data retained but dependencies unavailable for fresh inference | B6: missing items identified individually; saved historical output visibly limited, independent work usable; no silent network assumption or recalculation |
| Reconnect versus replace, 1.1.10 | R7: reconnect exact S1 reference/revision/bytes, reject changed bytes under same identity; admit replacement under new identity and derive only after authorized dependencies restored | B7: use real recovery controls with local files; reconnect retains source identity, replacement creates distinct evidence; inspect old/new receipts, invalidation and explanations |
| Per-investigation adoption, 1.4.21 | R8: correct L1 to L2; select A only; B retains applied L1 marked pending, cannot use revoked identity in fresh results; export/import B cannot promote stale L1 into current registry | B8: reopen previously closed B, see pending correction and applied revision; export/import into fresh and existing workspaces, inspect history; explicitly adopt later and verify only affected identity-derived outputs change |
| Import conflict, 1.4.19 | R9: import conflicting association; destination registry unchanged, source-local IDs intact, imported link pending; explicit confirm/reject/reversal leaves auditable history | B9: inspect both associations, evidence and approval histories; open unaffected evidence/history while identity-dependent current results unresolved; explicitly resolve, re-export/reimport, inspect outcome/history |
| Semantic preservation, 1.4.33 | R10: round-trip original propositions and every semantic relationship kind/direction/evidence/review state/revision; no automatic merge, reverse implication or person-Q attribution | B10: inspect each relation and corrected/rejected history after transfer; original propositions remain separate and P → Q is never displayed as Q → P |
| Trust/policy, issue #35 and policy/store contracts | R11: run mutation inventory and policy matrix; reject invalid inputs before effective state changes; same-ID substitutions fail, revoked dependencies never reactivate | B11: upload malformed, unsupported, tampered and unauthorized files into workspace with A/B; useful error, existing projects/registry unchanged; denied data never becomes fresh authorized evidence |
| Offline continuation/repeated transfer, 1.1.6/.8 | R12: import full eligible bundle without external resolver, add allowed evidence, export/import again and compare full history/identity/provenance | B12: offline fresh browser context imports full bundle, inspects originals and saved results, adds prepared evidence, resumes and transfers again; B remains independent |

## Four independent export combinations

Use a complete eligible-source fixture first, then the mixed S1/S2/S3 fixture.
For every row, assert saved historical conclusions, source-local people, source
references, and selected investigation history survive; unrelated C/registry
sentinels never appear. A fresh destination starts with no workspace identity
registry. A second pass uses the conflict destination.

| Original sources | Confirmed person links | Fresh offline destination |
| --- | --- | --- |
| Full bundle | Include relevant links | Permitted available originals inspectable; included approved associations inspectable/reusable under reviewed policy; no new match approval |
| Full bundle | Link references only | Originals inspectable; unavailable links flagged with recovery; identity-dependent current results unresolved |
| References only | Include relevant links | Original material missing unless genuinely available; approved links retained, historical analysis inspectable with source limitations |
| References only | Link references only | Both missing categories disclosed separately, source-local records/history retained, independent authorized continuation available |

Full-bundle coverage is conditional: S2 remains missing and S3 unbundled. It must
not say the mixed bundle is wholly offline resumable. References-only must omit
original payload bytes even when locally available; link-references-only must not
embed confirmed associations disguised as another field.

## Browser execution and evidence

Build using `scripts/build-pages.sh`; use the actual generated WASM and browser
assets, never a JavaScript imitation of Rust policy/inference/storage. Extend the
existing Playwright harness or a comparable repository-owned entrypoint. Download
files via browser events, import the downloaded bytes in a fresh browser context
with empty persistent storage, and reopen persistent state after a session restart.
For offline transfer load the locally served application shell/WASM first, then
block external network access and all source resolvers; preserve only the local
application origin needed to serve the app. Distinguish this from claiming the
website itself is installable and launchable without a network, which #35 does
not establish. Verify no remote source access satisfies the missing-source checks.

Record explicit before/after project and registry state for rejected imports,
conflicts and selected correction adoption. Download and reimport the resumed
project rather than inspecting only the original in-memory result. Capture a real
screenshot or recording of import, missing-source recovery and conflict inspection
under ignored `target/` for owner review. Screenshots supplement assertions and do
not establish owner acceptance.

## Existing coverage and remaining evidence

Source inspection only (none executed by this context):

- `apps/belief-cli/tests/person_belief.rs`: source identity/policy substitution,
  separate scores, quotation handling, collisions, missing/conflicting evidence
  and confidence validation. Useful R3/R11 prerequisites, no project round-trip.
- `apps/belief-cli/tests/person_belief_acceptance.rs`: unscored assessments and
  opposing evidence after review. Useful attribution regression checks, no R1–R12
  portable-project or person-link journey.
- `site/browser-test.mjs`: real WASM comparison, exact passage/offset retention,
  editable annotation export, bad input and mobile layout. Useful browser harness,
  no persistent multi-project import/recovery/registry journey.
- `apps/belief-cli/tests/workbench.rs`: native asset/session/origin/content-type/
  size/concurrency boundary; no portable-project acceptance.
- `docs/STORE.md` describes `inference_binding` and authorization snapshot checks;
  these establish lower-level prerequisites, not serialized project restoration.

Before closing #35, record the actual test paths/names and revision/results for
R1–R12/B1–B12, any legitimately shared scenarios, fixture identities and skipped
coverage. Run `cargo fmt --all -- --check`, workspace Clippy with warnings denied,
`cargo test --workspace`, `cargo run --quiet`, the production Pages build and
`npm --prefix site test`. After merge/deployment inspect the public Pages URL
separately and record deployed revision and observed behavior. A passing local
suite, a screenshot and owner acceptance are three distinct claims. No row in
this contract is cleared by the baseline's existing tests alone.
