# Architecture

`belief-lab` is a downstream epistemic system. It does not own detector, OCR, ASR, NER, scene, face, or voice truth.

## Dependency direction

```text
producer repositories
        |
        v
provenance-preserving evidence references
        |
        v
   belief-core
        |
        +----> belief-policy
        |
        v
 semantic-decision
        |
        +----> SemIf / future providers
        |
        v
 judgments + provider receipts
        |
        v
 inference-core / inference-baseline
```

Producer repositories remain authoritative for their outputs. `belief-lab` stores references and derivations, not replacement detector truth.

## Epistemic layers

The core model keeps these concepts distinct:

1. **Evidence** — a reference to producer-owned output with pinned source and producer revisions.
2. **Judgment** — a model interpretation of one or more pieces of evidence. Its score uses model-confidence semantics.
3. **Claim** — a proposition introduced by a user assertion, a judgment, or an explicit rule.
4. **Belief** — an inference result about a claim. Its score is explicitly either soft truth or a posterior probability.
5. **Derivation** — the inputs and rule that produced a belief.

A numerically identical score does not make two layers equivalent. Detector confidence, model confidence, similarity, soft truth, and posterior probability are separate score semantics.

## Provenance and evidence families

Every evidence reference requires:

- a source repository, record identifier, and pinned revision;
- a producer name and pinned revision;
- an evidence-family identifier.

Evidence families identify observations that share underlying information. Future inference engines must use them to avoid treating a derived track and each of its component observations as independent confirmations.

## Policy boundary

Domain vocabulary such as evidence and inference classes belongs to `belief-core`. `belief-policy` consumes that vocabulary and decides which classes a process may use.

This keeps authorization from becoming the owner of the domain model and lets future inference engines depend on a stable core while still requiring policy checks at execution boundaries.


## Semantic decision boundary

Semantic providers receive only an `AuthorizedDecisionRequest`: bounded state plus explicit evidence references and purposes that have passed `belief-policy`. Providers do not receive the store or corpus/database authority.

Provider output is recorded as a judgment. Direct option scores use `conditional_option_probability` semantics and therefore cannot be inserted as a belief value. Provider identity, model revision, runtime, prompt hash, readout, option scores, and authorization receipt remain available through the store explanation path.
