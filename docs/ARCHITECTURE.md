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

## Source-relative semantic evidence

Upstream semantic analysis remains source-relative. In particular, `nlp-stack` semantic-map concepts are corpus-local identities backed by source spans and producer provenance; they are not global world concepts and do not assert that their content is true.

`belief-lab` consumes references to those producer-owned semantic objects rather than copying embeddings, graph payloads, or corpus persistence. A decision request may resolve bounded source content for a specific authorized judgment, but the producer/corpus remains authoritative for the semantic-map object and its revision.

The later world-model step belongs on the epistemic side of this boundary: source-relative observations may be normalized into propositions, compared across sources, judged as support/contradiction/unknown, and incorporated into beliefs. Cross-corpus similarity alone must not collapse two corpus-local concepts into one world identity.

## Policy boundary

Domain vocabulary such as evidence and inference classes belongs to `belief-core`. `belief-policy` consumes that vocabulary and decides which classes a process may use.

This keeps authorization from becoming the owner of the domain model and lets future inference engines depend on a stable core while still requiring policy checks at execution boundaries.


## Semantic decision boundary

Semantic providers receive only an `AuthorizedDecisionRequest`: bounded state plus explicit evidence references and purposes that have passed `belief-policy`. Providers do not receive the store or corpus/database authority.

Provider output is recorded as a judgment. Direct option scores use `conditional_option_probability` semantics and therefore cannot be inserted as a belief value. Provider identity, model revision, runtime, prompt hash, readout, option scores, and authorization receipt remain available through the store explanation path.

## Application surfaces

`apps/belief-cli` exposes a shared Rust application library. The native `explain` CLI and the Pages WebAssembly binding call the same validation, policy, inference, store, and report functions. The JavaScript workbench handles file selection and display; it does not implement an alternative inference algorithm. SemIf remains a native-only dependency. Browser examples carry explicitly scripted judgments; real model requests execute through the local `decide` JSON/stdio boundary.

`repository-purpose.json` records the machine-readable ownership/exclusion boundary. Evidence-interchange rejects producer payload extensions; the one-decision request carries bounded content separately with its provenance-bearing references.
