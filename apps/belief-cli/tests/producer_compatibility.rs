//! Real producer-owned export compatibility, independent of sibling checkouts.
use belief_core::EvidenceClass;
use belief_policy::{AuthorizationProfile, PolicyConfig};
use belief_store::InMemoryBeliefStore;
use evidence_interchange::ValidatedEvidenceBatch;

const YOUTUBE_EXPORT: &str =
    include_str!("../../../fixtures/evidence-interchange/youtube-corpus-export-v1.json");

const PHILOSOPHY_EXPORT: &str =
    include_str!("../../../fixtures/evidence-interchange/philosophy-extractor-export-v1.json");

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

#[test]
fn document_text_has_its_own_class_and_respects_research_and_explicit_allowlists() {
    // Use the real import path to test the added class, rather than relabeling
    // document text as a transcript or investigator assertion in an adapter.
    let input = YOUTUBE_EXPORT.replace("\"class\": \"transcript\"", "\"class\": \"source_text\"");
    let batch = ValidatedEvidenceBatch::parse_json(&input).unwrap();
    assert!(batch
        .evidence()
        .iter()
        .all(|e| e.class == EvidenceClass::SourceText));
    for profile in ["semantic_research", "multimodal_research"] {
        let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", profile)]).unwrap();
        assert!(batch.clone().authorize(&policy).is_ok());
    }
    for pairs in [
        vec![("BELIEF_POLICY_PROFILE", "observe_only")],
        vec![
            ("BELIEF_POLICY_PROFILE", "semantic_research"),
            ("BELIEF_ALLOWED_EVIDENCE", "transcript"),
        ],
    ] {
        let policy = PolicyConfig::from_pairs(pairs).unwrap();
        assert!(batch.clone().authorize(&policy).is_err());
    }
    let denied = input.replace("source_text", "philosophical_truth");
    assert!(ValidatedEvidenceBatch::parse_json(&denied).is_err());
}

#[test]
fn philosophy_export_preserves_native_source_references_and_imported_parentage() {
    let batch = ValidatedEvidenceBatch::parse_json(PHILOSOPHY_EXPORT).unwrap();
    assert_eq!(batch.exporter_name(), "philosophy-extractor");
    assert_eq!(batch.evidence().len(), 10);
    assert_eq!(
        batch
            .evidence()
            .iter()
            .filter(|e| e.class == EvidenceClass::SourceText)
            .count(),
        7
    );
    assert_eq!(
        batch
            .evidence()
            .iter()
            .filter(|e| e.class == EvidenceClass::Metadata)
            .count(),
        3
    );
    let denied = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "observe_only")]).unwrap();
    assert!(batch.clone().authorize(&denied).is_err());
    let policy =
        PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
    let authorized = batch.clone().authorize(&policy).unwrap();
    let receipt = authorized.receipt().clone();
    let mut store = InMemoryBeliefStore::default();
    assert_eq!(
        store
            .insert_authorized_evidence_batch(authorized)
            .unwrap()
            .inserted,
        10
    );
    for evidence in batch.evidence() {
        assert_eq!(evidence.provenance.source.repository(), "document-search");
        assert_eq!(&store.evidence(&evidence.id).unwrap().value, evidence);
        if evidence.class == EvidenceClass::SourceText {
            assert_eq!(evidence.provenance.parent_evidence.len(), 1);
            let parent = store
                .evidence(evidence.provenance.parent_evidence.iter().next().unwrap())
                .unwrap();
            assert_eq!(parent.value.class, EvidenceClass::Metadata);
        }
    }
    assert_eq!(
        store.import_receipt(
            batch.exporter_name(),
            batch.revision(),
            AuthorizationProfile::SemanticResearch
        ),
        Some(&receipt)
    );
    let second = store
        .insert_authorized_evidence_batch(batch.authorize(&policy).unwrap())
        .unwrap();
    assert_eq!(second.inserted, 0);
    assert_eq!(second.already_present, 10);
}
