//! The policy → inference → store → explanation path shared by every CLI command.

use std::collections::BTreeSet;
use std::fmt;

use belief_core::JudgmentId;
use belief_policy::PolicyConfig;
use belief_store::{BeliefExplanation, InMemoryBeliefStore, StoreError};
use inference_baseline::BaselineInferenceEngine;
use inference_core::{AuthorizationError, InferenceEngine, InferenceError, InferenceRequest};

/// A belief that was authorized, inferred, stored, and read back through the store.
pub struct DerivedBelief {
    pub explanation: BeliefExplanation,
    pub selected_judgments: BTreeSet<JudgmentId>,
    pub ignored_correlated_judgments: BTreeSet<JudgmentId>,
}

#[derive(Debug)]
pub enum DeriveError {
    /// Policy refused the request; nothing was stored.
    Unauthorized(AuthorizationError),
    /// The engine produced no belief (for example only `unknown` judgments); nothing was stored.
    NoBelief(InferenceError),
    /// The store rejected a write; this indicates an inconsistent request, not a policy decision.
    Store(StoreError),
}

impl fmt::Display for DeriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized(error) => error.fmt(f),
            Self::NoBelief(error) => error.fmt(f),
            Self::Store(error) => write!(f, "store rejected the derivation: {error}"),
        }
    }
}

impl std::error::Error for DeriveError {}

/// Authorizes `request` and runs the baseline engine. Only then does it call
/// `record_judgments` (which inserts the request's judgments, plain or semantic), insert the
/// target claim and the result, and read the explanation back from the store.
///
/// Nothing is written unless authorization and inference both succeed. The evidence the
/// request uses must already be in `store`.
pub fn derive_belief(
    store: &mut InMemoryBeliefStore,
    policy: &PolicyConfig,
    request: InferenceRequest,
    record_judgments: impl FnOnce(&mut InMemoryBeliefStore) -> Result<(), StoreError>,
) -> Result<DerivedBelief, DeriveError> {
    let claim = request.claim().clone();
    let authorized = request
        .authorize(policy)
        .map_err(DeriveError::Unauthorized)?;
    let result = BaselineInferenceEngine
        .infer(&authorized)
        .map_err(DeriveError::NoBelief)?;
    let belief_id = result.belief().id().clone();
    let selected_judgments = result.selected_judgments().clone();
    let ignored_correlated_judgments = result.ignored_correlated_judgments().clone();

    record_judgments(store).map_err(DeriveError::Store)?;
    store.insert_claim(claim).map_err(DeriveError::Store)?;
    store
        .insert_inference_result(result)
        .map_err(DeriveError::Store)?;
    let explanation = store
        .explain_belief(&belief_id)
        .map_err(DeriveError::Store)?;

    Ok(DerivedBelief {
        explanation,
        selected_judgments,
        ignored_correlated_judgments,
    })
}
