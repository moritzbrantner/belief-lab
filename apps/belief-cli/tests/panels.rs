use belief_cli::pipeline::{derive_belief, panel_bases};
use belief_core::*;
use belief_policy::PolicyConfig;
use belief_store::InMemoryBeliefStore;
use inference_core::{InferenceRequest, TrustedInferenceRule};
use semantic_decision::*;
use std::collections::BTreeMap;

fn panel(family: &str) -> ModelPanel {
    let evidence = EvidenceRef::new(
        EvidenceId::new(format!("e:{family}")).unwrap(),
        None,
        EvidenceClass::Transcript,
        EvidenceFamilyId::new(family).unwrap(),
        None,
        Provenance::new(
            SourceRef::new("fixture", "scope", family, "sha256:source-v1").unwrap(),
            ProducerRef::new("fixture", "commit:v1", None, None).unwrap(),
            [],
        ),
    );
    let request = DecisionRequest::new(
        "same-id",
        serde_json::json!("bounded source"),
        "Does the source support the proposition?",
        ["supports", "contradicts", "unknown"]
            .into_iter()
            .map(|id| DecisionOption::new(id, id).unwrap())
            .collect(),
        InferenceClass::Preference,
        vec![DecisionEvidenceUse::new(
            evidence,
            EvidencePurpose::Corroboration,
        )],
    )
    .unwrap()
    .authorize(&policy())
    .unwrap();
    ModelPanel::new(
        format!("panel:{family}"),
        request,
        Proposition::new(
            EntityId::new("person:a").unwrap(),
            Predicate::new("prefers_customization").unwrap(),
            ObjectValue::Boolean(true),
        ),
        JudgmentSpecRef::new("test", "v1").unwrap(),
    )
    .unwrap()
}
fn policy() -> PolicyConfig {
    PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap()
}
fn receipt(panel: &ModelPanel, model: &str, outcome: &str) -> SemanticDecisionReceipt {
    SemanticDecisionReceipt::new(
        panel.target().request(),
        "fixture",
        "provider:v1",
        model,
        "model:v1",
        "test",
        "prompt:v1",
        "direct",
        ["supports", "contradicts", "unknown"]
            .into_iter()
            .map(|id| (id.into(), if id == outcome { 0.8 } else { 0.1 }))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}
#[test]
fn agreeing_models_contribute_once_and_all_members_remain_explainable() {
    let mut panel = panel("source:a");
    for model in ["c", "a", "b"] {
        panel
            .add(
                JudgmentId::new(model).unwrap(),
                receipt(&panel, model, "supports"),
            )
            .unwrap();
    }
    assert_eq!(panel.summary().supports, 3);
    assert_eq!(
        panel
            .members()
            .keys()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    let bases = panel_bases(&panel).unwrap();
    let claim = Claim::from_judgment(
        ClaimId::new("claim:1").unwrap(),
        bases[0].judgment.proposition().clone(),
        bases[0].judgment.id().clone(),
    );
    let mut store = InMemoryBeliefStore::default();
    for item in panel.target().request().evidence() {
        store.insert_evidence(item.evidence.clone()).unwrap();
    }
    store.insert_model_panel(panel).unwrap();
    let request = InferenceRequest::new(
        InferenceRunId::new("run:1").unwrap(),
        BeliefId::new("belief:1").unwrap(),
        TrustedInferenceRule::BaselinePreferenceV1,
        claim,
        bases,
    )
    .unwrap();
    let derived = derive_belief(&mut store, &policy(), request, |_| Ok(())).unwrap();
    assert_eq!(derived.selected_judgments.len(), 1);
    assert_eq!(derived.ignored_correlated_judgments.len(), 2);
    let retained = &derived.explanation.model_panels[0];
    assert_eq!(retained.members().len(), 3);
    assert_eq!(
        retained
            .members()
            .values()
            .next()
            .unwrap()
            .provenance()
            .decision()
            .model_revision(),
        "model:v1"
    );
}
#[test]
fn disagreement_unknown_request_binding_and_independent_sources() {
    let mut a = panel("source:a");
    let b = panel("source:b");
    assert_ne!(
        a.correlation_group().unwrap(),
        b.correlation_group().unwrap()
    );
    assert!(a
        .add(
            JudgmentId::new("mismatch").unwrap(),
            receipt(&b, "a", "supports")
        )
        .is_err());
    for (model, outcome) in [("a", "supports"), ("b", "contradicts"), ("c", "unknown")] {
        a.add(JudgmentId::new(model).unwrap(), receipt(&a, model, outcome))
            .unwrap();
    }
    assert_eq!(
        a.summary(),
        PanelSummary {
            members: 3,
            supports: 1,
            contradicts: 1,
            unknown: 1,
            disagreement: true
        }
    );
    assert_eq!(
        panel_bases(&a)
            .unwrap()
            .iter()
            .map(|b| b.correlation_group.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );
}
