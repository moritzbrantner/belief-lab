# Inference policy

`belief-lab` treats authorization as part of the inference boundary. Configuration is fail-closed: a profile defines a maximum capability set, and environment allowlists can only reduce that set.

## Profiles

| Profile | Evidence | Inference | Identity / cross-source |
| --- | --- | --- | --- |
| `observe_only` | user assertions, metadata | none | disabled |
| `semantic_research` | adds transcripts, OCR, object/scene detections, named entities | descriptive, preference | disabled |
| `multimodal_research` | adds face/voice track references | adds corpus-local entity links and cross-source associations | available only through separate opt-ins |

No shipped profile authorizes `sensitive_trait` or `real_world_identity` inference. Adding either requires a code change and review; changing `.env` is intentionally insufficient.

## Explicitly confirmed source-person links

`belief_policy::person_link` provides a separate boundary for an investigator's explicit,
non-biometric confirmation that two source-local person records refer to the same person
(Atlas Decisions 1.4.11 and 1.4.13). This is available under ordinary `semantic_research`
and `multimodal_research` policy; `observe_only` cannot confirm links. It does not enable
`real_world_identity` inference or change the existing automatic inference gates.

A `SourcePerson` is the tuple `(repository, scope_id, subject_id)`, not a name or a global
identity. A `PersonLinkProposal` requires distinct records, source-scoped support for both,
unique evidence IDs, and complete, acyclic support ancestry. Support is ordered by evidence ID.
A `HumanConfirmation` consumes that exact proposal together with the confirming investigator
and their nonempty reason. `PolicyConfig::confirm_person_link` authorizes every support record
and ancestor as `corroboration`, respects evidence/purpose allowlists and the descriptive-inference
allowlist, and permits only user assertions, metadata, transcripts, OCR and named-entity references.
Face/voice tracks, object/scene detections and biometric-derived support are refused even when
the older multimodal gates are enabled. Transcribed speech itself is not voiceprint matching.

The returned `ConfirmedPersonLink` is an immutable historical snapshot of the proposal,
confirmation and authorizing policy. An environment setting cannot manufacture that confirmation.
The calling application must obtain explicit consent and investigator identity at its trusted
interaction boundary; a constructor or imported actor string is not authentication or proof of
upstream identity truth. References retain producer-declared evidence classes and source identities;
this boundary does not inspect original source bytes or independently verify upstream declarations.

This prerequisite API does not store a shared registry, merge people or beliefs, resolve imported
link conflicts, or implement person linking in the browser. Before adopting or reusing saved links,
the future registry must check current policy, source validity, revocation and conflict state.
Imported confirmation history alone cannot override a destination link or authorize a new match.
Investigation-specific adoption of corrections and historical views remain obligations of #35–#37.
`cargo test -p belief-policy --test person_links` covers this policy boundary.

## Independent gates

The multimodal profile still starts with the high-risk gates off:

- `BELIEF_BIOMETRIC_EVIDENCE=reference_only` permits references to producer-owned face/voice tracks. Raw biometric embeddings remain outside `belief-lab`.
- `BELIEF_IDENTITY_RESOLUTION=corpus_local` permits links between corpus-local identities only.
- `BELIEF_ALLOW_CROSS_SOURCE_JOIN=true` permits inference that joins evidence from more than one source.

These gates are independent. Enabling one does not enable the others.

## Denial reasons

`PolicyConfig::evidence_decision` and `inference_decision` return the reason for a denial, for example that a class is outside the profile, an allowlist removed it, or a gate is off. The boolean `allows_*` helpers delegate to them, so a reason and a decision can never disagree. Sensitive-trait and real-world identity inference are denied before any profile or setting is consulted. `cargo run -- explain` shows these reasons.

## Evidence purpose

Authorization checks both an evidence class and the purpose for which it is used:

- `direct_support`: evidence directly supports a claim.
- `corroboration`: evidence may strengthen or weaken an existing claim but does not originate it.
- `entity_linking`: evidence participates in corpus-local entity resolution.

Face and voice track references can never be used as `direct_support`; even in multimodal research they are limited to corroboration and entity linking.

## Provenance

`BELIEF_REQUIRE_COMPLETE_PROVENANCE=true` is mandatory in the current implementation. Setting it to false is a configuration error rather than a supported lower authorization level.

This makes missing provenance a denial condition instead of a warning.

## Environment loading

`belief-policy` reads process environment variables but deliberately does not choose how `.env` files are loaded. A CLI or service may load a local `.env` before calling `PolicyConfig::from_env()`. This keeps policy evaluation deterministic and testable while allowing different deployment environments to provide configuration through their normal configuration mechanism.
