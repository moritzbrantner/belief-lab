use belief_core::{
    EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId, EvidenceRef, ProducerRef, Provenance,
    SourceRef,
};
use belief_policy::person_link::{HumanConfirmation, PersonLinkProposal, SourcePerson};
use belief_policy::PolicyConfig;

fn person(repository: &str, subject: &str) -> SourcePerson {
    SourcePerson::new(repository, "interview", EntityId::new(subject).unwrap()).unwrap()
}

fn evidence(repository: &str, subject: &str, class: EvidenceClass) -> EvidenceRef {
    EvidenceRef::new(
        EvidenceId::new(format!("{repository}:{subject}")).unwrap(),
        Some(EntityId::new(subject).unwrap()),
        class,
        EvidenceFamilyId::new(repository).unwrap(),
        None,
        Provenance::new(
            SourceRef::new(repository, "interview", subject, "git:source-v1").unwrap(),
            ProducerRef::new("producer", "git:producer-v1", None, None).unwrap(),
            [],
        ),
    )
}

fn proposal(class: EvidenceClass) -> PersonLinkProposal {
    PersonLinkProposal::new(
        person("source-a", "speaker"),
        person("source-b", "speaker"),
        vec![
            evidence("source-a", "speaker", class),
            evidence("source-b", "speaker", class),
        ],
    )
    .unwrap()
}

fn confirmation(proposal: PersonLinkProposal) -> HumanConfirmation {
    HumanConfirmation::new(
        proposal,
        "investigator",
        "Inspected both source biographies and stable identifiers",
    )
    .unwrap()
}

#[test]
fn explicit_confirmation_is_bound_to_exact_people_evidence_and_policy() {
    let policy =
        PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
    let proposal = proposal(EvidenceClass::Metadata);
    let confirmed = policy
        .confirm_person_link(confirmation(proposal.clone()))
        .unwrap();
    assert_eq!(confirmed.proposal(), &proposal);
    assert_eq!(confirmed.confirmation().confirmed_by(), "investigator");
    assert_eq!(
        confirmed.confirmation().reason(),
        "Inspected both source biographies and stable identifiers"
    );
    assert_eq!(confirmed.policy(), &policy);
    assert_ne!(confirmed.proposal().left(), confirmed.proposal().right());
}

#[test]
fn profiles_and_evidence_restrictions_are_enforced_without_enabling_identity_inference() {
    for profile in ["observe_only", "semantic_research", "multimodal_research"] {
        let policy = PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", profile)]).unwrap();
        let result = policy.confirm_person_link(confirmation(proposal(EvidenceClass::Metadata)));
        assert_eq!(result.is_ok(), profile != "observe_only", "{profile}");
        for class in [
            belief_core::InferenceClass::RealWorldIdentity,
            belief_core::InferenceClass::SensitiveTrait,
        ] {
            assert!(!policy.allows_inference(class));
        }
    }
    for pairs in [
        vec![
            ("BELIEF_POLICY_PROFILE", "semantic_research"),
            ("BELIEF_ALLOWED_EVIDENCE", "transcript"),
        ],
        vec![
            ("BELIEF_POLICY_PROFILE", "semantic_research"),
            ("BELIEF_ALLOWED_EVIDENCE_PURPOSES", "direct_support"),
        ],
        vec![
            ("BELIEF_POLICY_PROFILE", "semantic_research"),
            ("BELIEF_ALLOWED_INFERENCES", "preference"),
        ],
    ] {
        let policy = PolicyConfig::from_pairs(pairs).unwrap();
        assert!(policy
            .confirm_person_link(confirmation(proposal(EvidenceClass::Metadata)))
            .is_err());
    }
}

#[test]
fn biometric_and_detector_evidence_cannot_authorize_a_person_link() {
    let policy = PolicyConfig::from_pairs([
        ("BELIEF_POLICY_PROFILE", "multimodal_research"),
        ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
        ("BELIEF_IDENTITY_RESOLUTION", "corpus_local"),
        ("BELIEF_ALLOW_CROSS_SOURCE_JOIN", "true"),
    ])
    .unwrap();
    for class in [
        EvidenceClass::FaceTrackReference,
        EvidenceClass::VoiceTrackReference,
        EvidenceClass::ObjectDetection,
        EvidenceClass::SceneDetection,
    ] {
        assert!(policy
            .confirm_person_link(confirmation(proposal(class)))
            .is_err());
    }
    for class in [
        EvidenceClass::Metadata,
        EvidenceClass::Transcript,
        EvidenceClass::Ocr,
        EvidenceClass::NamedEntity,
        EvidenceClass::UserAssertion,
    ] {
        assert!(policy
            .confirm_person_link(confirmation(proposal(class)))
            .is_ok());
    }
}

#[test]
fn missing_mismatched_duplicate_or_changed_support_is_rejected() {
    let policy =
        PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
    let mut missing = proposal(EvidenceClass::Metadata).evidence().to_vec();
    missing.pop();
    assert!(PersonLinkProposal::new(
        person("source-a", "speaker"),
        person("source-b", "speaker"),
        missing
    )
    .is_err());
    assert!(PersonLinkProposal::new(
        person("source-a", "speaker"),
        person("source-a", "speaker"),
        vec![]
    )
    .is_err());
    for mutation in ["subject", "scope", "repository", "duplicate", "provenance"] {
        let mut support = proposal(EvidenceClass::Metadata).evidence().to_vec();
        match mutation {
            "subject" => support[1].subject = Some(EntityId::new("other").unwrap()),
            "scope" => {
                support[1].provenance.source =
                    SourceRef::new("source-b", "other", "speaker", "git:v1").unwrap()
            }
            "repository" => {
                support[1].provenance.source =
                    SourceRef::new("other", "interview", "speaker", "git:v1").unwrap()
            }
            "duplicate" => support[1].id = support[0].id.clone(),
            "provenance" => {
                support[1]
                    .provenance
                    .parent_evidence
                    .insert(EvidenceId::new("unavailable-parent").unwrap());
            }
            _ => unreachable!(),
        }
        let result = PersonLinkProposal::new(
            person("source-a", "speaker"),
            person("source-b", "speaker"),
            support,
        )
        .and_then(|proposal| policy.confirm_person_link(confirmation(proposal)));
        assert!(result.is_err(), "{mutation}");
    }
    assert!(HumanConfirmation::new(proposal(EvidenceClass::Metadata), "", "reason").is_err());
    assert!(HumanConfirmation::new(proposal(EvidenceClass::Metadata), "analyst", " ").is_err());
    assert!(SourcePerson::new(" ", "scope", EntityId::new("person").unwrap()).is_err());
}

#[test]
fn textual_lineage_is_preserved_but_biometric_ancestors_are_refused() {
    let policy = PolicyConfig::from_pairs([
        ("BELIEF_POLICY_PROFILE", "multimodal_research"),
        ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
    ])
    .unwrap();
    for class in [
        EvidenceClass::Transcript,
        EvidenceClass::VoiceTrackReference,
    ] {
        let mut support = proposal(EvidenceClass::Metadata).evidence().to_vec();
        let parent = evidence("transcription", "speaker", class);
        support[0]
            .provenance
            .parent_evidence
            .insert(parent.id.clone());
        support.push(parent);
        let original = PersonLinkProposal::new(
            person("source-a", "speaker"),
            person("source-b", "speaker"),
            support.clone(),
        )
        .unwrap();
        support.reverse();
        let reordered = PersonLinkProposal::new(
            person("source-a", "speaker"),
            person("source-b", "speaker"),
            support,
        )
        .unwrap();
        assert_eq!(original, reordered);
        assert_eq!(
            policy.confirm_person_link(confirmation(original)).is_ok(),
            class == EvidenceClass::Transcript
        );
    }
    let mut support = proposal(EvidenceClass::Transcript).evidence().to_vec();
    let (left, right) = support.split_at_mut(1);
    left[0]
        .provenance
        .parent_evidence
        .insert(right[0].id.clone());
    right[0]
        .provenance
        .parent_evidence
        .insert(left[0].id.clone());
    assert!(PersonLinkProposal::new(
        person("source-a", "speaker"),
        person("source-b", "speaker"),
        support
    )
    .is_err());
}
