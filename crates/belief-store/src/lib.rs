use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use belief_core::{
    Belief, BeliefId, Claim, ClaimId, ClaimOrigin, Derivation, EvidenceId, EvidenceRef, Judgment,
    JudgmentId,
};
use belief_policy::AuthorizationProfile;
use evidence_interchange::{AuthorizedEvidenceBatch, EvidenceImportReceipt};
use inference_core::{AuthorizationReceipt, InferenceResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validity {
    Active,
    Invalidated { reason: String },
}

impl Validity {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stored<T> {
    pub value: T,
    pub validity: Validity,
}

impl<T> Stored<T> {
    fn active(value: T) -> Self {
        Self {
            value,
            validity: Validity::Active,
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemoryBeliefStore {
    evidence: BTreeMap<EvidenceId, Stored<EvidenceRef>>,
    judgments: BTreeMap<JudgmentId, Stored<Judgment>>,
    claims: BTreeMap<ClaimId, Stored<Claim>>,
    beliefs: BTreeMap<BeliefId, Stored<Belief>>,
    derivations: BTreeMap<BeliefId, Derivation>,
    authorizations: BTreeMap<BeliefId, AuthorizationReceipt>,
    import_receipts: BTreeMap<(String, AuthorizationProfile), EvidenceImportReceipt>,
}

impl InMemoryBeliefStore {
    pub fn insert_authorized_evidence_batch(
        &mut self,
        batch: AuthorizedEvidenceBatch,
    ) -> Result<EvidenceImportOutcome, StoreError> {
        let (evidence, receipt) = batch.into_parts();
        let receipt_key = (
            receipt.batch_revision().to_string(),
            receipt.profile(),
        );

        if let Some(existing) = self.import_receipts.get(&receipt_key) {
            if existing != &receipt {
                return Err(StoreError::ImportReceiptConflict {
                    revision: receipt.batch_revision().to_string(),
                    profile: receipt.profile(),
                });
            }
        }

        let mut available = self
            .evidence
            .iter()
            .filter(|(_, stored)| stored.validity.is_active())
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let mut inserted = 0usize;
        let mut already_present = 0usize;

        for item in &evidence {
            if let Some(existing) = self.evidence.get(&item.id) {
                if !existing.validity.is_active() {
                    return Err(StoreError::InvalidDependency {
                        kind: "evidence",
                        id: item.id.to_string(),
                    });
                }
                if existing.value != *item {
                    return Err(StoreError::EvidenceConflict(item.id.clone()));
                }
                already_present += 1;
            } else {
                for parent in &item.provenance.parent_evidence {
                    if !available.contains(parent) {
                        return Err(StoreError::Missing {
                            kind: "evidence",
                            id: parent.to_string(),
                        });
                    }
                }
                inserted += 1;
            }

            available.insert(item.id.clone());
        }

        for item in evidence {
            if !self.evidence.contains_key(&item.id) {
                self.evidence
                    .insert(item.id.clone(), Stored::active(item));
            }
        }

        self.import_receipts.insert(receipt_key, receipt);

        Ok(EvidenceImportOutcome {
            inserted,
            already_present,
        })
    }

    pub fn import_receipt(
        &self,
        batch_revision: &str,
        profile: AuthorizationProfile,
    ) -> Option<&EvidenceImportReceipt> {
        self.import_receipts
            .get(&(batch_revision.to_string(), profile))
    }

    pub fn insert_evidence(&mut self, evidence: EvidenceRef) -> Result<(), StoreError> {
        if self.evidence.contains_key(&evidence.id) {
            return Err(StoreError::Duplicate {
                kind: "evidence",
                id: evidence.id.to_string(),
            });
        }

        for parent in &evidence.provenance.parent_evidence {
            self.require_active_evidence(parent)?;
        }

        self.evidence
            .insert(evidence.id.clone(), Stored::active(evidence));
        Ok(())
    }

    pub fn insert_judgment(&mut self, judgment: Judgment) -> Result<(), StoreError> {
        if self.judgments.contains_key(&judgment.id) {
            return Err(StoreError::Duplicate {
                kind: "judgment",
                id: judgment.id.to_string(),
            });
        }

        for evidence in &judgment.evidence {
            self.require_active_evidence(evidence)?;
        }

        self.judgments
            .insert(judgment.id.clone(), Stored::active(judgment));
        Ok(())
    }

    pub fn insert_claim(&mut self, claim: Claim) -> Result<(), StoreError> {
        if self.claims.contains_key(&claim.id) {
            return Err(StoreError::Duplicate {
                kind: "claim",
                id: claim.id.to_string(),
            });
        }

        match &claim.origin {
            ClaimOrigin::UserAssertion(evidence) => self.require_active_evidence(evidence)?,
            ClaimOrigin::Judgment(judgment) => self.require_active_judgment(judgment)?,
            ClaimOrigin::Rule { input_claims, .. } => {
                for input in input_claims {
                    self.require_active_claim(input)?;
                }
            }
        }

        self.claims.insert(claim.id.clone(), Stored::active(claim));
        Ok(())
    }

    pub fn insert_inference_result(&mut self, result: InferenceResult) -> Result<(), StoreError> {
        let (belief, derivation, selected_judgments, _, authorization) = result.into_parts();

        if self.beliefs.contains_key(&belief.id) {
            return Err(StoreError::Duplicate {
                kind: "belief",
                id: belief.id.to_string(),
            });
        }

        self.require_active_claim(&belief.claim)?;

        if derivation.belief != belief.id {
            return Err(StoreError::InconsistentResult(
                "derivation belief id does not match belief".into(),
            ));
        }
        if derivation.inference_run != belief.inference_run {
            return Err(StoreError::InconsistentResult(
                "derivation inference run does not match belief".into(),
            ));
        }
        if derivation.judgments != selected_judgments {
            return Err(StoreError::InconsistentResult(
                "derivation judgments do not match selected judgments".into(),
            ));
        }

        for evidence in &derivation.evidence {
            self.require_active_evidence(evidence)?;
        }
        for judgment in &derivation.judgments {
            self.require_active_judgment(judgment)?;
        }
        for claim in &derivation.claims {
            self.require_active_claim(claim)?;
        }

        self.authorizations.insert(belief.id.clone(), authorization);
        self.derivations.insert(belief.id.clone(), derivation);
        self.beliefs
            .insert(belief.id.clone(), Stored::active(belief));
        Ok(())
    }

    pub fn revoke_evidence(
        &mut self,
        evidence: &EvidenceId,
        reason: impl Into<String>,
    ) -> Result<(), StoreError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(StoreError::EmptyInvalidationReason);
        }

        let stored = self
            .evidence
            .get_mut(evidence)
            .ok_or_else(|| StoreError::Missing {
                kind: "evidence",
                id: evidence.to_string(),
            })?;

        if stored.validity.is_active() {
            stored.validity = Validity::Invalidated {
                reason: reason.clone(),
            };
        }

        self.propagate_invalidation();
        Ok(())
    }

    pub fn explain_belief(&self, belief: &BeliefId) -> Result<BeliefExplanation, StoreError> {
        let stored_belief =
            self.beliefs
                .get(belief)
                .cloned()
                .ok_or_else(|| StoreError::Missing {
                    kind: "belief",
                    id: belief.to_string(),
                })?;
        let claim = self
            .claims
            .get(&stored_belief.value.claim)
            .cloned()
            .ok_or_else(|| StoreError::Missing {
                kind: "claim",
                id: stored_belief.value.claim.to_string(),
            })?;
        let derivation =
            self.derivations
                .get(belief)
                .cloned()
                .ok_or_else(|| StoreError::Missing {
                    kind: "derivation",
                    id: belief.to_string(),
                })?;

        let judgments = derivation
            .judgments
            .iter()
            .filter_map(|id| self.judgments.get(id).cloned())
            .collect();
        let evidence = derivation
            .evidence
            .iter()
            .filter_map(|id| self.evidence.get(id).cloned())
            .collect();
        let input_claims = derivation
            .claims
            .iter()
            .filter_map(|id| self.claims.get(id).cloned())
            .collect();

        let authorization =
            self.authorizations
                .get(belief)
                .cloned()
                .ok_or_else(|| StoreError::Missing {
                    kind: "authorization",
                    id: belief.to_string(),
                })?;

        Ok(BeliefExplanation {
            belief: stored_belief,
            claim,
            derivation,
            authorization,
            judgments,
            evidence,
            input_claims,
        })
    }

    pub fn evidence(&self, id: &EvidenceId) -> Option<&Stored<EvidenceRef>> {
        self.evidence.get(id)
    }

    pub fn judgment(&self, id: &JudgmentId) -> Option<&Stored<Judgment>> {
        self.judgments.get(id)
    }

    pub fn claim(&self, id: &ClaimId) -> Option<&Stored<Claim>> {
        self.claims.get(id)
    }

    pub fn belief(&self, id: &BeliefId) -> Option<&Stored<Belief>> {
        self.beliefs.get(id)
    }

    fn require_active_evidence(&self, id: &EvidenceId) -> Result<(), StoreError> {
        require_active(self.evidence.get(id), "evidence", id.to_string())
    }

    fn require_active_judgment(&self, id: &JudgmentId) -> Result<(), StoreError> {
        require_active(self.judgments.get(id), "judgment", id.to_string())
    }

    fn require_active_claim(&self, id: &ClaimId) -> Result<(), StoreError> {
        require_active(self.claims.get(id), "claim", id.to_string())
    }

    fn propagate_invalidation(&mut self) {
        loop {
            let mut changed = false;

            let invalid_judgments = self
                .judgments
                .iter()
                .filter(|(_, stored)| stored.validity.is_active())
                .filter_map(|(id, stored)| {
                    stored
                        .value
                        .evidence
                        .iter()
                        .find(|evidence| self.is_invalid_evidence(evidence))
                        .map(|evidence| {
                            (
                                id.clone(),
                                format!("evidence dependency {evidence} was invalidated"),
                            )
                        })
                })
                .collect::<Vec<_>>();
            changed |= invalidate_many(&mut self.judgments, invalid_judgments);

            let invalid_claims = self
                .claims
                .iter()
                .filter(|(_, stored)| stored.validity.is_active())
                .filter_map(|(id, stored)| {
                    self.invalid_claim_dependency(&stored.value)
                        .map(|reason| (id.clone(), reason))
                })
                .collect::<Vec<_>>();
            changed |= invalidate_many(&mut self.claims, invalid_claims);

            let invalid_beliefs = self
                .beliefs
                .iter()
                .filter(|(_, stored)| stored.validity.is_active())
                .filter_map(|(id, stored)| {
                    self.invalid_belief_dependency(id, &stored.value)
                        .map(|reason| (id.clone(), reason))
                })
                .collect::<Vec<_>>();
            changed |= invalidate_many(&mut self.beliefs, invalid_beliefs);

            if !changed {
                break;
            }
        }
    }

    fn invalid_claim_dependency(&self, claim: &Claim) -> Option<String> {
        match &claim.origin {
            ClaimOrigin::UserAssertion(evidence) if self.is_invalid_evidence(evidence) => {
                Some(format!("evidence dependency {evidence} was invalidated"))
            }
            ClaimOrigin::Judgment(judgment) if self.is_invalid_judgment(judgment) => {
                Some(format!("judgment dependency {judgment} was invalidated"))
            }
            ClaimOrigin::Rule { input_claims, .. } => input_claims
                .iter()
                .find(|claim| self.is_invalid_claim(claim))
                .map(|claim| format!("claim dependency {claim} was invalidated")),
            _ => None,
        }
    }

    fn invalid_belief_dependency(&self, id: &BeliefId, belief: &Belief) -> Option<String> {
        if self.is_invalid_claim(&belief.claim) {
            return Some(format!("claim dependency {} was invalidated", belief.claim));
        }

        let derivation = self.derivations.get(id)?;
        if let Some(evidence) = derivation
            .evidence
            .iter()
            .find(|evidence| self.is_invalid_evidence(evidence))
        {
            return Some(format!("evidence dependency {evidence} was invalidated"));
        }
        if let Some(judgment) = derivation
            .judgments
            .iter()
            .find(|judgment| self.is_invalid_judgment(judgment))
        {
            return Some(format!("judgment dependency {judgment} was invalidated"));
        }
        if let Some(claim) = derivation
            .claims
            .iter()
            .find(|claim| self.is_invalid_claim(claim))
        {
            return Some(format!("claim dependency {claim} was invalidated"));
        }

        None
    }

    fn is_invalid_evidence(&self, id: &EvidenceId) -> bool {
        self.evidence
            .get(id)
            .is_some_and(|stored| !stored.validity.is_active())
    }

    fn is_invalid_judgment(&self, id: &JudgmentId) -> bool {
        self.judgments
            .get(id)
            .is_some_and(|stored| !stored.validity.is_active())
    }

    fn is_invalid_claim(&self, id: &ClaimId) -> bool {
        self.claims
            .get(id)
            .is_some_and(|stored| !stored.validity.is_active())
    }
}

fn require_active<T>(
    stored: Option<&Stored<T>>,
    kind: &'static str,
    id: String,
) -> Result<(), StoreError> {
    match stored {
        None => Err(StoreError::Missing { kind, id }),
        Some(stored) if !stored.validity.is_active() => {
            Err(StoreError::InvalidDependency { kind, id })
        }
        Some(_) => Ok(()),
    }
}

fn invalidate_many<K: Ord, T>(
    values: &mut BTreeMap<K, Stored<T>>,
    invalidations: Vec<(K, String)>,
) -> bool {
    let mut changed = false;
    for (id, reason) in invalidations {
        if let Some(stored) = values.get_mut(&id) {
            if stored.validity.is_active() {
                stored.validity = Validity::Invalidated { reason };
                changed = true;
            }
        }
    }
    changed
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceImportOutcome {
    pub inserted: usize,
    pub already_present: usize,
}

pub struct BeliefExplanation {
    pub belief: Stored<Belief>,
    pub claim: Stored<Claim>,
    pub derivation: Derivation,
    pub authorization: AuthorizationReceipt,
    pub judgments: Vec<Stored<Judgment>>,
    pub evidence: Vec<Stored<EvidenceRef>>,
    pub input_claims: Vec<Stored<Claim>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    Duplicate { kind: &'static str, id: String },
    Missing { kind: &'static str, id: String },
    InvalidDependency { kind: &'static str, id: String },
    InconsistentResult(String),
    EvidenceConflict(EvidenceId),
    ImportReceiptConflict {
        revision: String,
        profile: AuthorizationProfile,
    },
    EmptyInvalidationReason,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { kind, id } => write!(f, "{kind} {id} already exists"),
            Self::Missing { kind, id } => write!(f, "{kind} {id} does not exist"),
            Self::InvalidDependency { kind, id } => {
                write!(f, "{kind} dependency {id} is invalidated")
            }
            Self::InconsistentResult(reason) => {
                write!(f, "inconsistent inference result: {reason}")
            }
            Self::EvidenceConflict(id) => {
                write!(f, "evidence {id} already exists with different provenance or content")
            }
            Self::ImportReceiptConflict { revision, profile } => write!(
                f,
                "evidence import {revision} already has a different receipt for profile {}",
                profile.as_str()
            ),
            Self::EmptyInvalidationReason => f.write_str("invalidation reason cannot be empty"),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use belief_core::{
        BeliefId, ClaimId, EntityId, EvidenceClass, EvidenceFamilyId, EvidencePurpose,
        InferenceClass, InferenceRunId, JudgmentOutcome, JudgmentSpecRef, ObjectValue, Predicate,
        ProducerRef, Proposition, Provenance, Score, ScoreSemantics, SourceRef,
    };
    use belief_policy::PolicyConfig;
    use evidence_interchange::ValidatedEvidenceBatch;
    use inference_baseline::BaselineInferenceEngine;
    use inference_core::{EvidenceUse, InferenceEngine, InferenceRequest, JudgmentBasis};

    use super::*;

    fn evidence(id: &str) -> EvidenceRef {
        EvidenceRef::new(
            EvidenceId::new(id).unwrap(),
            Some(EntityId::new("person:alice").unwrap()),
            EvidenceClass::Transcript,
            EvidenceFamilyId::new(format!("family:{id}")).unwrap(),
            None,
            Provenance::new(
                SourceRef::new(
                    "youtube-corpus",
                    "video:1",
                    format!("video:1#{id}"),
                    "sha256:source-v1",
                )
                .unwrap(),
                ProducerRef::new(
                    "audio-analysis",
                    "commit:abc",
                    Some("whisper".into()),
                    Some("config:def".into()),
                )
                .unwrap(),
                [],
            ),
        )
    }

    fn proposition() -> Proposition {
        Proposition::new(
            EntityId::new("person:alice").unwrap(),
            Predicate::new("prefers_customization").unwrap(),
            ObjectValue::Boolean(true),
        )
    }

    fn judgment(evidence: &EvidenceRef) -> Judgment {
        Judgment::new(
            JudgmentId::new("judgment:1").unwrap(),
            proposition(),
            JudgmentOutcome::Supports,
            Score::new(0.8, ScoreSemantics::ModelConfidence).unwrap(),
            [evidence.id.clone()],
            JudgmentSpecRef::new("preference-evidence", "v1").unwrap(),
            "jev:fixture",
        )
        .unwrap()
    }

    fn inference_result(
        evidence: &EvidenceRef,
        judgment: &Judgment,
        claim: Claim,
    ) -> InferenceResult {
        let basis = JudgmentBasis::new(
            judgment.clone(),
            EvidenceFamilyId::new("correlation:utterance:1").unwrap(),
            vec![EvidenceUse::new(
                evidence.clone(),
                EvidencePurpose::Corroboration,
            )],
        )
        .unwrap();
        let request = InferenceRequest::new(
            InferenceRunId::new("run:1").unwrap(),
            BeliefId::new("belief:1").unwrap(),
            InferenceClass::Preference,
            claim,
            "baseline:preference:v1",
            vec![basis],
        )
        .unwrap();
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        let authorized = request.authorize(&policy).unwrap();

        BaselineInferenceEngine.infer(&authorized).unwrap()
    }

    fn import_json(source_revision: &str) -> String {
        format!(
            r#"{{
  "schema": "belief_evidence_interchange",
  "schemaVersion": 1,
  "exporter": {{
    "name": "youtube-corpus",
    "revision": "git:exporter-1"
  }},
  "revision": "batch:1",
  "evidence": [
    {{
      "id": "evidence:transcript:1",
      "subject": "corpus-person:1",
      "class": "transcript",
      "familyId": "family:utterance:1",
      "source": {{
        "repository": "youtube-corpus",
        "scopeId": "video:1",
        "recordId": "transcript-segment:1",
        "revision": "{source_revision}"
      }},
      "producer": {{
        "name": "audio-analysis",
        "revision": "git:audio-1",
        "model": "whisper"
      }},
      "score": {{
        "value": 0.9,
        "semantics": "model_confidence"
      }}
    }}
  ]
}}"#
        )
    }

    #[test]
    fn authorized_evidence_import_is_idempotent_and_persists_receipt() {
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        let batch =
            ValidatedEvidenceBatch::parse_json(&import_json("sha256:source-v1")).unwrap();
        let authorized = batch.authorize(&policy).unwrap();

        let mut store = InMemoryBeliefStore::default();
        let first = store
            .insert_authorized_evidence_batch(authorized.clone())
            .unwrap();
        let second = store
            .insert_authorized_evidence_batch(authorized)
            .unwrap();

        assert_eq!(first.inserted, 1);
        assert_eq!(first.already_present, 0);
        assert_eq!(second.inserted, 0);
        assert_eq!(second.already_present, 1);
        assert!(store
            .import_receipt("batch:1", AuthorizationProfile::SemanticResearch)
            .is_some());
    }

    #[test]
    fn same_evidence_id_cannot_silently_change_source_revision() {
        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        let first = ValidatedEvidenceBatch::parse_json(&import_json("sha256:source-v1"))
            .unwrap()
            .authorize(&policy)
            .unwrap();
        let changed = ValidatedEvidenceBatch::parse_json(&import_json("sha256:source-v2"))
            .unwrap()
            .authorize(&policy)
            .unwrap();

        let mut store = InMemoryBeliefStore::default();
        store.insert_authorized_evidence_batch(first).unwrap();

        assert!(matches!(
            store.insert_authorized_evidence_batch(changed),
            Err(StoreError::EvidenceConflict(_))
        ));
    }

    #[test]
    fn explanation_reaches_pinned_producer_provenance() {
        let evidence = evidence("evidence:1");
        let judgment = judgment(&evidence);
        let claim = Claim::from_judgment(
            ClaimId::new("claim:1").unwrap(),
            proposition(),
            judgment.id.clone(),
        );
        let result = inference_result(&evidence, &judgment, claim.clone());

        let mut store = InMemoryBeliefStore::default();
        store.insert_evidence(evidence).unwrap();
        store.insert_judgment(judgment).unwrap();
        store.insert_claim(claim).unwrap();
        store.insert_inference_result(result).unwrap();

        let explanation = store
            .explain_belief(&BeliefId::new("belief:1").unwrap())
            .unwrap();

        assert_eq!(explanation.evidence.len(), 1);
        assert_eq!(
            explanation.evidence[0].value.provenance.producer.name,
            "audio-analysis"
        );
        assert_eq!(
            explanation.evidence[0].value.provenance.producer.revision,
            "commit:abc"
        );
        assert_eq!(explanation.belief.validity, Validity::Active);
        assert_eq!(
            explanation.authorization.profile(),
            AuthorizationProfile::SemanticResearch
        );
        assert_eq!(explanation.authorization.evidence_uses().len(), 1);
    }

    #[test]
    fn evidence_revocation_invalidates_judgment_claim_and_belief() {
        let evidence = evidence("evidence:1");
        let evidence_id = evidence.id.clone();
        let judgment = judgment(&evidence);
        let judgment_id = judgment.id.clone();
        let claim = Claim::from_judgment(
            ClaimId::new("claim:1").unwrap(),
            proposition(),
            judgment.id.clone(),
        );
        let claim_id = claim.id.clone();
        let result = inference_result(&evidence, &judgment, claim.clone());

        let mut store = InMemoryBeliefStore::default();
        store.insert_evidence(evidence).unwrap();
        store.insert_judgment(judgment).unwrap();
        store.insert_claim(claim).unwrap();
        store.insert_inference_result(result).unwrap();

        store
            .revoke_evidence(&evidence_id, "source record was removed")
            .unwrap();

        assert!(!store.evidence(&evidence_id).unwrap().validity.is_active());
        assert!(!store.judgment(&judgment_id).unwrap().validity.is_active());
        assert!(!store.claim(&claim_id).unwrap().validity.is_active());
        assert!(!store
            .belief(&BeliefId::new("belief:1").unwrap())
            .unwrap()
            .validity
            .is_active());

        let explanation = store
            .explain_belief(&BeliefId::new("belief:1").unwrap())
            .unwrap();
        assert!(!explanation.belief.validity.is_active());
        assert!(!explanation.evidence[0].validity.is_active());
    }

    #[test]
    fn invalidated_dependencies_cannot_be_reused() {
        let evidence = evidence("evidence:1");
        let evidence_id = evidence.id.clone();

        let mut store = InMemoryBeliefStore::default();
        store.insert_evidence(evidence.clone()).unwrap();
        store
            .revoke_evidence(&evidence_id, "source record was removed")
            .unwrap();

        assert!(matches!(
            store.insert_judgment(judgment(&evidence)),
            Err(StoreError::InvalidDependency {
                kind: "evidence",
                ..
            })
        ));
    }

    #[test]
    fn rule_claim_invalidation_propagates_transitively() {
        let evidence = evidence("evidence:1");
        let evidence_id = evidence.id.clone();
        let direct = Claim::from_user_assertion(
            ClaimId::new("claim:direct").unwrap(),
            proposition(),
            evidence_id.clone(),
        );
        let derived = Claim::from_rule(
            ClaimId::new("claim:derived").unwrap(),
            proposition(),
            "rule:derived:v1",
            [direct.id.clone()],
        )
        .unwrap();

        let mut store = InMemoryBeliefStore::default();
        store.insert_evidence(evidence).unwrap();
        store.insert_claim(direct.clone()).unwrap();
        store.insert_claim(derived.clone()).unwrap();

        store
            .revoke_evidence(&evidence_id, "source record was removed")
            .unwrap();

        assert!(!store.claim(&direct.id).unwrap().validity.is_active());
        assert!(!store.claim(&derived.id).unwrap().validity.is_active());
    }
}
