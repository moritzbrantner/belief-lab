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

## Semantic judgments

Before inference, an optional semantic-decision provider may assess a bounded question over admitted evidence. The provider boundary uses the same inference/evidence policy checks as the inference boundary, including cross-source restrictions.

A semantic provider emits a `Judgment`, not a `Belief`. Its selected-option score is tagged `conditional_option_probability`; this preserves SemIf-style direct option-score semantics without relabeling them as calibrated model confidence, soft truth, or a posterior probability.

The store retains the semantic-decision authorization and provider receipt for any such judgment selected by a later belief derivation.

## Baseline semantics

The baseline engine intentionally produces `ScoreSemantics::SoftTruth`, never `PosteriorProbability`.

For each selected supporting or contradicting judgment it uses the judgment score (either model confidence or a conditional option probability) as a signed heuristic signal, averages the selected signals, and maps the result to `[0, 1]` around a neutral value of `0.5`. The resulting value is still soft truth; it is not a calibration claim about the input scores.

This gives the project a deterministic reference implementation for tests and explanations without pretending that model confidence is a Bayesian likelihood.


## Authorization receipts

Authorization is persisted as provenance rather than discarded after the gate.

An authorized request carries a receipt containing the selected policy profile, inference class, source scopes, evidence ids and purposes, and whether the request crossed source scopes. `InferenceResult` fields are private and a result can only be created through its validating constructor with an `AuthorizedInferenceRequest`.

The store persists that receipt with the belief and exposes it through `explain_belief`. This prevents callers from bypassing policy by constructing a result with a public struct literal and makes the authorization context auditable alongside the evidence derivation.
