# Architecture

## Direction

`belief-lab` is downstream of corpus and analysis repositories. It must not become a competing detector, transcriber, NER system, or source database.

```text
source material
    |
    v
moenarch-corpus-core ------------------- portable identity / provenance
    |
    +--> scenedetect-rs ---------------- scene evidence
    +--> visual-analysis ---------------- object / face / OCR evidence
    +--> audio-analysis ----------------- speech / speaker evidence
    +--> nlp-stack ---------------------- entity / relation evidence
                  |
                  v
            corpus owners
          (youtube-corpus, ...)
                  |
        versioned evidence exports
                  |
                  v
              belief-lab
          observations / links
          judgments / claims
          inference / explanations
```

## Epistemic layers

The project keeps these concepts distinct:

1. **Observation** — a producer emitted something, such as an object detection or NER relation.
2. **Evidence** — the observation plus source references and exact producer provenance.
3. **Entity link** — an uncertain hypothesis that two local identities refer to one entity.
4. **Judgment** — a bounded model assessment, later including Jev decisions.
5. **Claim** — a proposition derived from evidence under an explicit rule.
6. **Belief** — a later inference result with named probabilistic/soft-truth semantics.

A numeric value always carries semantics. `detection_confidence`, `jev_confidence`, `heuristic_strength`, `soft_truth`, and `posterior_probability` are not interchangeable.

## Cross-repository contracts

The first slice reads the JSON shape of:

- `source_span_interchange@1` for source-grounded document/transcript spans;
- `media_evidence@1` for video scene/OCR/SponsorBlock context;
- `belief_evidence_bundle@1` as the belief-lab-owned envelope for additional normalized observations.

The producer contracts stay authoritative in their producer/corpus repositories. `belief-lab` validates and references them; it does not rewrite their source truth.

## Correlation

Derived artifacts retain `correlationGroup`. For example, six OCR observations and the OCR track derived from them are not seven independent confirmations. The current baseline exposes correlation groups in explanations and deliberately avoids probabilistic combination. Later inference engines must define how correlated evidence is handled before producing posterior semantics.

## First baseline rule

`baseline.entity-link-substitution.v1` performs only this transformation:

```text
relation(local_subject, predicate, object)
entity_link(local_subject, canonical_subject)
    ->
claim(canonical_subject, predicate, object)
```

Its output is a `heuristic_strength` equal to the weaker input score. This is useful for proving the provenance graph while avoiding unjustified probability arithmetic.
