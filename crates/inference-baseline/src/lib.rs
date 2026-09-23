use std::collections::{BTreeMap, BTreeSet};

use belief_core::{Derivation, EvidenceId, JudgmentId, JudgmentOutcome, Score, ScoreSemantics};
use inference_core::{
    AuthorizedInferenceRequest, InferenceEngine, InferenceError, InferenceResult, JudgmentBasis,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct BaselineInferenceEngine;

impl InferenceEngine for BaselineInferenceEngine {
    fn name(&self) -> &'static str {
        "baseline:v1"
    }

    fn infer(
        &self,
        request: &AuthorizedInferenceRequest,
    ) -> Result<InferenceResult, InferenceError> {
        let authorized_request = request;
        let request = authorized_request.request();

        let mut selected_by_group = BTreeMap::<_, &JudgmentBasis>::new();
        let mut usable_judgments = BTreeSet::new();

        for basis in request.bases() {
            if basis.judgment.outcome() == JudgmentOutcome::Unknown {
                continue;
            }

            usable_judgments.insert(basis.judgment.id().clone());
            selected_by_group
                .entry(basis.correlation_group.clone())
                .and_modify(|selected| {
                    if should_replace(selected, basis) {
                        *selected = basis;
                    }
                })
                .or_insert(basis);
        }

        if selected_by_group.is_empty() {
            return Err(InferenceError::NoUsableSignals);
        }

        let mut signed_sum = 0.0;
        let mut selected_judgments = BTreeSet::new();
        let mut evidence = BTreeSet::<EvidenceId>::new();

        for basis in selected_by_group.values() {
            let confidence = basis.judgment.confidence().value();
            signed_sum += match basis.judgment.outcome() {
                JudgmentOutcome::Supports => confidence,
                JudgmentOutcome::Contradicts => -confidence,
                JudgmentOutcome::Unknown => unreachable!("unknown judgments are filtered above"),
            };
            selected_judgments.insert(basis.judgment.id().clone());
            evidence.extend(
                basis
                    .evidence
                    .iter()
                    .map(|evidence_use| evidence_use.evidence.id.clone()),
            );
        }

        let ignored_correlated_judgments = usable_judgments
            .difference(&selected_judgments)
            .cloned()
            .collect::<BTreeSet<JudgmentId>>();

        let signed_mean = signed_sum / selected_judgments.len() as f64;
        let soft_truth = (0.5 + signed_mean / 2.0).clamp(0.0, 1.0);

        let belief = belief_core::Belief::new(
            request.belief_id().clone(),
            request.claim().id.clone(),
            Score::new(soft_truth, ScoreSemantics::SoftTruth)?,
            request.run_id().clone(),
        )?;

        let derivation = Derivation::new(
            belief.id().clone(),
            request.rule_id().to_string(),
            evidence,
            selected_judgments.clone(),
            [],
            request.run_id().clone(),
        )?;

        InferenceResult::new(
            authorized_request,
            belief,
            derivation,
            selected_judgments,
            ignored_correlated_judgments,
        )
    }
}

fn should_replace(selected: &JudgmentBasis, candidate: &JudgmentBasis) -> bool {
    let selected_confidence = selected.judgment.confidence().value();
    let candidate_confidence = candidate.judgment.confidence().value();

    candidate_confidence > selected_confidence
        || (candidate_confidence == selected_confidence
            && candidate.judgment.id().as_str() < selected.judgment.id().as_str())
}

#[cfg(test)]
mod tests {
    use belief_core::{
        BeliefId, Claim, ClaimId, EntityId, EvidenceClass, EvidenceFamilyId, EvidenceId,
        EvidencePurpose, EvidenceRef, InferenceRunId, Judgment, JudgmentId, JudgmentSpecRef,
        ObjectValue, Predicate, ProducerRef, Proposition, Provenance, Score, ScoreSemantics,
        SourceRef,
    };
    use belief_policy::PolicyConfig;
    use inference_core::{EvidenceUse, InferenceRequest, JudgmentBasis, TrustedInferenceRule};

    use super::*;

    fn proposition() -> Proposition {
        Proposition::new(
            EntityId::new("person:alice").unwrap(),
            Predicate::new("prefers_customization").unwrap(),
            ObjectValue::Boolean(true),
        )
    }

    fn claim() -> Claim {
        Claim::from_user_assertion(
            ClaimId::new("claim:target").unwrap(),
            proposition(),
            EvidenceId::new("assertion:target").unwrap(),
        )
    }

    fn basis(
        judgment_id: &str,
        evidence_id: &str,
        group: &str,
        outcome: JudgmentOutcome,
        confidence: f64,
    ) -> JudgmentBasis {
        let evidence = EvidenceRef::new(
            EvidenceId::new(evidence_id).unwrap(),
            Some(EntityId::new("person:alice").unwrap()),
            EvidenceClass::Transcript,
            EvidenceFamilyId::new(format!("source-family:{evidence_id}")).unwrap(),
            None,
            Provenance::new(
                SourceRef::new(
                    "youtube-corpus",
                    "video:1",
                    format!("video:1#{evidence_id}"),
                    "sha256:video",
                )
                .unwrap(),
                ProducerRef::new("audio-analysis", "commit:1", None, Some("config:1".into()))
                    .unwrap(),
                [],
            ),
        );

        let judgment = Judgment::new(
            JudgmentId::new(judgment_id).unwrap(),
            proposition(),
            outcome,
            Score::new(confidence, ScoreSemantics::ModelConfidence).unwrap(),
            [evidence.id.clone()],
            JudgmentSpecRef::new("preference-evidence", "v1").unwrap(),
            "jev:fixture",
        )
        .unwrap();

        JudgmentBasis::new(
            judgment,
            EvidenceFamilyId::new(group).unwrap(),
            vec![EvidenceUse::new(evidence, EvidencePurpose::Corroboration)],
        )
        .unwrap()
    }

    fn authorized(bases: Vec<JudgmentBasis>) -> inference_core::AuthorizedInferenceRequest {
        let request = InferenceRequest::new(
            InferenceRunId::new("run:1").unwrap(),
            BeliefId::new("belief:1").unwrap(),
            TrustedInferenceRule::BaselinePreferenceV1,
            claim(),
            bases,
        )
        .unwrap();

        let policy =
            PolicyConfig::from_pairs([("BELIEF_POLICY_PROFILE", "semantic_research")]).unwrap();
        request.authorize(&policy).unwrap()
    }

    #[test]
    fn correlated_judgments_contribute_at_most_once() {
        let request = authorized(vec![
            basis(
                "judgment:strong",
                "evidence:1",
                "family:same-utterance",
                JudgmentOutcome::Supports,
                0.9,
            ),
            basis(
                "judgment:derived",
                "evidence:2",
                "family:same-utterance",
                JudgmentOutcome::Supports,
                0.8,
            ),
            basis(
                "judgment:independent",
                "evidence:3",
                "family:other-source",
                JudgmentOutcome::Contradicts,
                0.7,
            ),
        ]);

        let result = BaselineInferenceEngine.infer(&request).unwrap();

        assert_eq!(result.selected_judgments().len(), 2);
        assert!(result
            .selected_judgments()
            .contains(&JudgmentId::new("judgment:strong").unwrap()));
        assert!(result
            .ignored_correlated_judgments()
            .contains(&JudgmentId::new("judgment:derived").unwrap()));
        assert_eq!(
            result.belief().value().semantics(),
            ScoreSemantics::SoftTruth
        );
        assert!((result.belief().value().value() - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn inference_is_deterministic_across_input_order() {
        let first = authorized(vec![
            basis(
                "judgment:a",
                "evidence:1",
                "family:1",
                JudgmentOutcome::Supports,
                0.6,
            ),
            basis(
                "judgment:b",
                "evidence:2",
                "family:2",
                JudgmentOutcome::Supports,
                0.8,
            ),
        ]);
        let second = authorized(vec![
            basis(
                "judgment:b",
                "evidence:2",
                "family:2",
                JudgmentOutcome::Supports,
                0.8,
            ),
            basis(
                "judgment:a",
                "evidence:1",
                "family:1",
                JudgmentOutcome::Supports,
                0.6,
            ),
        ]);

        let engine = BaselineInferenceEngine;
        let first_result = engine.infer(&first).unwrap();
        let second_result = engine.infer(&second).unwrap();

        assert_eq!(
            first_result.belief().value().value(),
            second_result.belief().value().value()
        );
        assert_eq!(
            first_result.selected_judgments(),
            second_result.selected_judgments()
        );
    }

    #[test]
    fn unknown_only_input_is_not_silently_converted_into_a_belief() {
        let request = authorized(vec![basis(
            "judgment:unknown",
            "evidence:1",
            "family:1",
            JudgmentOutcome::Unknown,
            0.9,
        )]);

        assert_eq!(
            BaselineInferenceEngine.infer(&request),
            Err(InferenceError::NoUsableSignals)
        );
    }
}
