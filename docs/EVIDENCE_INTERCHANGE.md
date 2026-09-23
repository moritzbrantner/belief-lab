# Evidence interchange v1

`belief_evidence_interchange@1` is the import boundary between corpus/analysis systems and `belief-lab`.

It is intentionally a **reference contract**, not a universal media schema.

## Why references instead of copied payloads

Producer repositories remain authoritative for their own data:

- `audio-analysis` owns ASR/speaker outputs;
- `visual-analysis` owns object/OCR/face outputs;
- `scenedetect-rs` owns scene detection;
- `nlp-stack` owns named entities and semantic analysis;
- corpus repositories such as `youtube-corpus` own aligned persistence and stable record identities.

The interchange therefore does not copy transcript text, bounding boxes, embeddings, detector-specific metadata, or semantic graph payloads into a new belief-specific representation.

Each evidence item carries only enough information for `belief-lab` to identify and reason about the evidence safely:

- stable evidence id;
- optional corpus-local subject id;
- evidence class;
- correlation/evidence family;
- exact source repository, scope, record id, and revision;
- exact producer revision plus optional model/config identity;
- optional score with explicit semantics;
- parent evidence ids for derived evidence.

Future evidence-bundle resolvers can retrieve bounded source content when a Jev or other judgment provider actually needs it.

## Envelope

A v1 batch uses:

```json
{
  "schema": "belief_evidence_interchange",
  "schemaVersion": 1,
  "exporter": {
    "name": "youtube-corpus",
    "revision": "git:<exact exporter revision>"
  },
  "revision": "<exact batch revision>",
  "evidence": []
}
```

Unknown schema/version combinations are rejected.

The exporter revision is separate from every source and producer revision. This lets a consumer distinguish:

1. the code that exported the batch;
2. the revision of the source record being referenced;
3. the revision/model/configuration that produced that source record.

## Parentage and ordering

`parentEvidenceIds` records derivation between imported evidence records.

For v1, parent references must be self-contained within the batch. The importer:

- rejects missing parents;
- rejects cycles;
- topologically orders evidence before returning it.

This lets a store insert parents before derived records without guessing about dependency order.

## Score semantics

Imported evidence may currently carry:

- `detector_confidence`;
- `model_confidence`;
- `similarity`.

The importer rejects:

- `soft_truth`;
- `posterior_probability`.

Those are inference-result semantics and may only be created inside `belief-lab` by an inference engine with an authorized request.

A numeric value such as `0.9` therefore cannot silently change meaning when it crosses a repository boundary.

## Policy admission

Parsing and validating a batch does not itself authorize the data for the active process.

A validated batch must be admitted through `belief-policy`. The selected profile controls which evidence classes may enter:

- `semantic_research` rejects face/voice-track references;
- `multimodal_research` still rejects them unless `BELIEF_BIOMETRIC_EVIDENCE=reference_only` is explicitly enabled.

Successful admission issues an `EvidenceImportReceipt` containing the policy profile, exporter revision, batch revision, imported evidence ids, and evidence classes.

## Store semantics

`belief-store` accepts an `AuthorizedEvidenceBatch`.

Imports are idempotent:

- importing the same evidence again is a no-op for already-present identical records;
- the import receipt is retained;
- reusing the same evidence id with different provenance or source revision is rejected;
- invalidated evidence is never silently reactivated.

This creates a stable contract for a future PostgreSQL implementation.

## Relationship to other interchange contracts

`philosophy-extractor` already experiments with `source_span_interchange@1` and media-specific evidence structures. Those contracts preserve text/media details for philosophical extraction.

`belief_evidence_interchange@1` is deliberately narrower. An adapter may map those richer contracts into belief evidence references, but `belief-lab` does not copy their domain schemas or become authoritative for them.

The fixture at `fixtures/evidence-interchange/youtube-multimodal-v1.json` demonstrates references to transcript, NER, scene, OCR, face-track, and voice-track evidence exported by a corpus while preserving the original producer ownership.
