# Roadmap

## Milestone 1 — provenance-first explanation

- [x] Versioned `belief_evidence_bundle@1` envelope.
- [x] Validate existing `source_span_interchange@1` and `media_evidence@1` shapes.
- [x] Dogfood `moenarch-corpus-core` source/asset/segment identifiers during import.
- [x] Preserve explicit score semantics and correlation groups.
- [x] Deterministic baseline inference over a synthetic multimodal fixture.
- [x] `belief validate`, `belief infer`, and `belief explain` CLI surfaces.
- [x] GitHub Pages architecture/explanation showcase using `github-pages-template`.

## Milestone 2 — corpus adapter

- [ ] Add a source-first `youtube-corpus` exporter/adapter once its scene/OCR evidence stack is integrated.
- [ ] Import face/voice tracks and cross-video cluster hypotheses without copying biometric embeddings.
- [ ] Add deterministic invalidation when producer revision, model, input hash, or config hash changes.
- [ ] Add `belief diff <run-a> <run-b>` for downstream impact inspection.

## Milestone 3 — typed judgments

- [ ] Add a provider interface for Jev in shadow mode.
- [ ] Persist question definition/version, exact evidence set, provider/model revision, and returned distribution.
- [ ] Keep Jev confidence separate from calibrated evidence likelihoods and final inference semantics.

## Milestone 4 — inference engines

- [ ] Extract an `InferenceEngine` interface.
- [ ] Keep the deterministic baseline as the reference behavior.
- [ ] Add Scallop behind an adapter and compare provenance behavior on identical fixtures.
- [ ] Add conflicting- and correlated-evidence fixtures before any engine may emit posterior semantics.

## Milestone 5 — durable store

- [ ] Add PostgreSQL as the authoritative belief/evidence store.
- [ ] Keep graph engines and vector indexes as rebuildable projections.
- [ ] Add migrations for entities, evidence, judgments, claims, beliefs, and derivations.
