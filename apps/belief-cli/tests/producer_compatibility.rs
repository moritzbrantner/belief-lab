//! Real producer-owned export compatibility, independent of sibling checkouts.
use belief_core::EvidenceClass;
use belief_policy::{AuthorizationProfile, PolicyConfig};
use belief_store::InMemoryBeliefStore;
use evidence_interchange::ValidatedEvidenceBatch;

const YOUTUBE_EXPORT: &str =
    include_str!("../../../fixtures/evidence-interchange/youtube-corpus-export-v1.json");

#[test]
fn youtube_export_preserves_references_and_receipts_through_real_store_import() {
    let batch = ValidatedEvidenceBatch::parse_json(YOUTUBE_EXPORT).unwrap();
    assert_eq!(batch.exporter_name(), "youtube-corpus");
    assert_eq!(batch.evidence().len(), 7);
    let policy =
        PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
    let authorized = batch.clone().authorize(&policy).unwrap();
    let expected_receipt = authorized.receipt().clone();
    let mut store = InMemoryBeliefStore::default();
    let first = store.insert_authorized_evidence_batch(authorized).unwrap();
    assert_eq!(first.inserted, 7);
    assert_eq!(first.already_present, 0);
    for evidence in batch.evidence() {
        assert_eq!(evidence.class, EvidenceClass::Transcript);
        let retained = store.evidence(&evidence.id).unwrap();
        assert_eq!(&retained.value, evidence);
        assert!(retained.validity.is_active());
    }
    assert_eq!(
        store.import_receipt(
            batch.exporter_name(),
            batch.revision(),
            AuthorizationProfile::SemanticResearch,
        ),
        Some(&expected_receipt)
    );
    let repeated = store
        .insert_authorized_evidence_batch(batch.authorize(&policy).unwrap())
        .unwrap();
    assert_eq!(repeated.inserted, 0);
    assert_eq!(repeated.already_present, 7);
}

#[test]
fn producer_export_does_not_bypass_current_evidence_policy() {
    let batch = ValidatedEvidenceBatch::parse_json(YOUTUBE_EXPORT).unwrap();
    let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "observe_only")]).unwrap();
    assert!(batch.authorize(&policy).is_err());
}
