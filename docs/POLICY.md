# Inference policy

`belief-lab` treats authorization as part of the inference boundary. Configuration is fail-closed: a profile defines a maximum capability set, and environment allowlists can only reduce that set.

## Profiles

| Profile | Evidence | Inference | Identity / cross-source |
| --- | --- | --- | --- |
| `observe_only` | user assertions, metadata | none | disabled |
| `semantic_research` | adds transcripts, OCR, object/scene detections, named entities | descriptive, preference | disabled |
| `multimodal_research` | adds face/voice track references | adds corpus-local entity links and cross-source associations | available only through separate opt-ins |

No shipped profile authorizes `sensitive_trait` or `real_world_identity` inference. Adding either requires a code change and review; changing `.env` is intentionally insufficient.

## Independent gates

The multimodal profile still starts with the high-risk gates off:

- `BELIEF_BIOMETRIC_EVIDENCE=reference_only` permits references to producer-owned face/voice tracks. Raw biometric embeddings remain outside `belief-lab`.
- `BELIEF_IDENTITY_RESOLUTION=corpus_local` permits links between corpus-local identities only.
- `BELIEF_ALLOW_CROSS_SOURCE_JOIN=true` permits inference that joins evidence from more than one source.

These gates are independent. Enabling one does not enable the others.

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
