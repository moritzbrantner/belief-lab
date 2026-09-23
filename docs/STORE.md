# Provenance store contract

The first store implementation is deliberately in-memory. Its purpose is to define the invariants that a later PostgreSQL adapter must preserve.

## Write ordering

The store accepts data in epistemic order:

```text
evidence
   |
   v
judgment
   |
   v
claim
   |
   v
belief + derivation
```

A write is rejected when a referenced dependency is missing or already invalidated.

## Invalidation

Source evidence is not silently deleted from history. `revoke_evidence` marks it invalid with a reason and propagates invalidity transitively:

- judgments using the evidence become invalid;
- claims originating from those judgments or evidence become invalid;
- rule-derived claims depending on invalid claims become invalid;
- beliefs whose target claim or derivation inputs are invalid become invalid.

Propagation reaches a fixed point so multi-step rule chains cannot remain active after an upstream source is revoked.

## Explanation

`explain_belief` remains available for active and invalidated beliefs. It returns:

- the belief and its validity;
- the target claim;
- the derivation;
- selected judgments;
- semantic-decision authorization/provider receipts for selected semantic judgments;
- selected evidence with pinned source/producer provenance;
- direct input claims.

This keeps invalidation auditable rather than erasing why a previous belief existed.

## Persistence boundary

A PostgreSQL implementation should match these semantics before becoming authoritative. In particular it must not:

- allow a judgment to reference missing or invalid evidence;
- allow a belief to omit its derivation;
- physically cascade-delete the explanation chain by default;
- treat an invalidated dependency as usable evidence for a new inference run.


Semantic judgments are inserted through `insert_semantic_judgment`, which validates their evidence dependencies before storing the judgment and its provider provenance together. Revocation still propagates through the ordinary judgment dependency graph; semantic provenance is retained for audit.
