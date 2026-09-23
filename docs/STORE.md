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

## Authorization input binding

An identifier is not proof that a stored value is the value supplied for authorization. Before inserting an inference result, the store compares the complete target claim (including proposition and origin), selected judgments, and selected evidence with their authorized values. Evidence comparisons include source/producer provenance, class, family, subject, score, and parent references. A same-ID substitution is an inconsistent result, not a new authorized interpretation.

`InferenceRequest::new` revalidates public judgment bases before making them immutable and rejects conflicting evidence values under the same ID, within or across bases. Identical shared evidence is allowed; every requested purpose still passes through policy authorization.

`AuthorizationReceipt::authorized_claim()` and `authorized_evidence()` expose read-only snapshots of the authorized inputs. The snapshot contains one value per evidence ID and is shared by receipt clones, inference results, and explanations rather than being deep-copied each time. It records what was authorized; it does not assert that the claim is true. Only evidence selected into the derivation must be present and active in the store. Ignored correlated inputs remain in the authorization snapshot but do not become additional stored dependencies.

All dependency checks and value comparisons complete before the belief, derivation, or authorization receipt is inserted. A future database adapter must preserve this compare-and-write boundary transactionally. Authorization snapshots are historical provenance; their presence does not make subsequently revoked evidence usable again.

The public receipt implements `PartialEq`, not `Eq`, because its complete evidence values include scores. The existing `InferenceResult::into_parts` shape is unchanged.

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
- the inference authorization receipt and its bound input snapshot;
- selected judgments;
- semantic-decision authorization/provider receipts for selected semantic judgments;
- selected evidence with pinned source/producer provenance;
- direct input claims.

This keeps invalidation auditable rather than erasing why a previous belief existed.

## Persistence boundary

A PostgreSQL implementation should match these semantics before becoming authoritative. In particular it must not:

- allow a judgment to reference missing or invalid evidence;
- accept same-ID substitutions for the authorized target claim, selected judgments, or selected evidence;
- allow a belief to omit its derivation;
- physically cascade-delete the explanation chain by default;
- treat an invalidated dependency as usable evidence for a new inference run.

Semantic judgments are inserted through `insert_semantic_judgment`, which validates their evidence dependencies before storing the judgment and its provider provenance together. Revocation still propagates through the ordinary judgment dependency graph; semantic provenance is retained for audit.

## Regression checks

`cargo test -p belief-store --test inference_binding` exercises the public API against request mutation, conflicting same-ID references, nine evidence drift variants, claim proposition/origin drift, valid shared evidence, atomic rejection, and post-revocation explanations.

`cargo test -p inference-core authorization_snapshot_is_deduplicated_and_shared_across_receipt_clones` checks snapshot deduplication and pointer sharing at 1, 16, and 256 judgment bases. This is a deterministic structural ratchet, not a wall-clock performance threshold.
