# Inference execution

Inference engines do not receive unrestricted evidence.

## Flow

```text
EvidenceRef + purpose
        |
        v
JudgmentBasis
        |
        v
InferenceRequest
        |
        v
belief-policy authorization
        |
        v
AuthorizedInferenceRequest
        |
        v
InferenceEngine
        |
        +--> Belief
        +--> Derivation
```

Only `AuthorizedInferenceRequest` may be passed to an `InferenceEngine`. This makes policy checking part of the type boundary rather than a convention followed by individual engines.

## Source scopes

Each source reference carries both a canonical `scope_id` and a more specific `record_id`.

For example, OCR and transcript evidence from different spans of the same video can share `scope_id = video:123` while keeping distinct record identifiers. Policy therefore distinguishes a multimodal join within one source from a join across separate source records.

Cross-source inference is denied unless `BELIEF_ALLOW_CROSS_SOURCE_JOIN=true`.

## Correlation groups

A `JudgmentBasis` has an explicit correlation group. This is separate from the evidence family's producer-level grouping because one model judgment may consume several related observations.

The deterministic baseline engine allows at most one non-unknown judgment from each correlation group to affect a belief. The strongest confidence is selected; ties use the judgment id for deterministic selection.

This is a conservative double-counting defense, not a claim of statistical independence between the remaining groups.

## Baseline semantics

The baseline engine intentionally produces `ScoreSemantics::SoftTruth`, never `PosteriorProbability`.

For each selected supporting or contradicting judgment it uses the model confidence as a signed heuristic signal, averages the selected signals, and maps the result to `[0, 1]` around a neutral value of `0.5`.

This gives the project a deterministic reference implementation for tests and explanations without pretending that model confidence is a Bayesian likelihood.
