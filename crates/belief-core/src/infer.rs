use std::collections::{HashMap, HashSet};

use crate::model::*;
use crate::validate::ValidatedBundle;

pub fn infer_baseline(validated: &ValidatedBundle) -> Vec<Claim> {
    let observations = &validated.bundle.observations;
    let links = observations
        .iter()
        .filter(|observation| observation.kind == ObservationKindV1::EntityLink)
        .filter(|observation| observation.predicate == "same_entity")
        .filter_map(|observation| {
            observation
                .object
                .as_ref()
                .map(|object| (observation, object))
        });

    let relations = observations
        .iter()
        .filter(|observation| observation.kind == ObservationKindV1::Relation)
        .filter_map(|observation| {
            observation
                .object
                .as_ref()
                .map(|object| (observation, object))
        })
        .collect::<Vec<_>>();

    let mut claims = Vec::new();
    for (link, canonical_subject) in links {
        for (relation, relation_object) in &relations {
            if relation.subject != link.subject {
                continue;
            }
            let strength = relation
                .score
                .as_ref()
                .map(|score| score.value)
                .unwrap_or(1.0)
                .min(link.score.as_ref().map(|score| score.value).unwrap_or(1.0));
            claims.push(Claim {
                id: claim_id(canonical_subject, &relation.predicate, relation_object),
                subject: canonical_subject.to_string(),
                predicate: relation.predicate.clone(),
                object: relation_object.to_string(),
                support: EvidenceScoreV1 {
                    value: strength,
                    semantics: ScoreSemanticsV1::HeuristicStrength,
                },
                inference_rule: "baseline.entity-link-substitution.v1".to_string(),
                evidence_ids: vec![relation.id.clone(), link.id.clone()],
            });
        }
    }
    claims.sort_by(|left, right| left.id.cmp(&right.id));
    claims.dedup_by(|left, right| left.id == right.id);
    claims
}

pub fn explain_claim(
    validated: &ValidatedBundle,
    claim: &Claim,
) -> Result<ExplanationNode, BeliefError> {
    let observations = validated
        .bundle
        .observations
        .iter()
        .map(|observation| (observation.id.as_str(), observation))
        .collect::<HashMap<_, _>>();
    let mut visiting = HashSet::new();
    let children = claim
        .evidence_ids
        .iter()
        .map(|id| explain_evidence(validated, &observations, id, &mut visiting))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ExplanationNode {
        id: claim.id.clone(),
        kind: "claim".to_string(),
        label: format!("{} {} {}", claim.subject, claim.predicate, claim.object),
        detail: Some(format!(
            "Derived by {}. The score is heuristic support strength, not a posterior probability.",
            claim.inference_rule
        )),
        score: Some(claim.support.clone()),
        producer: Some(ProducerV1 {
            name: "belief-lab".to_string(),
            revision: env!("CARGO_PKG_VERSION").to_string(),
        }),
        correlation_group: None,
        children,
    })
}

fn explain_evidence(
    validated: &ValidatedBundle,
    observations: &HashMap<&str, &ObservationV1>,
    id: &str,
    visiting: &mut HashSet<String>,
) -> Result<ExplanationNode, BeliefError> {
    if !visiting.insert(id.to_string()) {
        return Ok(ExplanationNode {
            id: id.to_string(),
            kind: "cycle".to_string(),
            label: "cycle suppressed".to_string(),
            detail: None,
            score: None,
            producer: None,
            correlation_group: None,
            children: Vec::new(),
        });
    }

    let node = if let Some(observation) = observations.get(id) {
        let children = observation
            .evidence_refs
            .iter()
            .map(|reference| explain_evidence(validated, observations, reference, visiting))
            .collect::<Result<Vec<_>, _>>()?;
        ExplanationNode {
            id: observation.id.clone(),
            kind: format!("{:?}", observation.kind).to_lowercase(),
            label: format!(
                "{} {} {}",
                observation.subject,
                observation.predicate,
                observation.object.as_deref().unwrap_or("")
            )
            .trim()
            .to_string(),
            detail: None,
            score: observation.score.clone(),
            producer: Some(observation.provenance.clone()),
            correlation_group: observation.correlation_group.clone(),
            children,
        }
    } else if let Some(span) = validated
        .bundle
        .source_spans
        .spans
        .iter()
        .find(|span| span.id == id)
    {
        ExplanationNode {
            id: span.id.clone(),
            kind: "source_span".to_string(),
            label: span.text.clone(),
            detail: Some(format!(
                "source {} · sequence {}",
                span.source_id, span.sequence
            )),
            score: None,
            producer: Some(validated.bundle.source_spans.producer.clone()),
            correlation_group: None,
            children: Vec::new(),
        }
    } else if let Some(source) = validated
        .bundle
        .source_spans
        .sources
        .iter()
        .find(|source| source.id == id)
    {
        ExplanationNode {
            id: source.id.clone(),
            kind: "source".to_string(),
            label: source.title.clone().unwrap_or_else(|| source.id.clone()),
            detail: Some(source.revision.clone()),
            score: None,
            producer: Some(validated.bundle.source_spans.producer.clone()),
            correlation_group: None,
            children: Vec::new(),
        }
    } else if let Some((batch, scene)) = validated.bundle.media_evidence.iter().find_map(|batch| {
        batch
            .scenes
            .iter()
            .find(|scene| scene.id == id)
            .map(|scene| (batch, scene))
    }) {
        ExplanationNode {
            id: scene.id.clone(),
            kind: "scene".to_string(),
            label: format!(
                "scene {} · {:.2}s–{:.2}s",
                scene.scene_index, scene.start_seconds, scene.end_seconds
            ),
            detail: Some(format!("media evidence revision {}", batch.revision)),
            score: None,
            producer: Some(ProducerV1 {
                name: scene.provenance.processor.clone(),
                revision: scene.provenance.processor_version.clone(),
            }),
            correlation_group: None,
            children: Vec::new(),
        }
    } else if let Some((batch, ocr)) = validated.bundle.media_evidence.iter().find_map(|batch| {
        batch
            .ocr_observations
            .iter()
            .find(|ocr| ocr.id == id)
            .map(|ocr| (batch, ocr))
    }) {
        ExplanationNode {
            id: ocr.id.clone(),
            kind: "ocr_observation".to_string(),
            label: ocr.text.clone(),
            detail: Some(format!("media evidence revision {}", batch.revision)),
            score: ocr.confidence.map(|value| EvidenceScoreV1 {
                value,
                semantics: ScoreSemanticsV1::DetectionConfidence,
            }),
            producer: Some(ProducerV1 {
                name: ocr.provenance.processor.clone(),
                revision: ocr.provenance.processor_version.clone(),
            }),
            correlation_group: None,
            children: Vec::new(),
        }
    } else if let Some((batch, track)) = validated.bundle.media_evidence.iter().find_map(|batch| {
        batch
            .ocr_tracks
            .iter()
            .find(|track| track.id == id)
            .map(|track| (batch, track))
    }) {
        let mut refs = track.observation_ids.clone();
        refs.extend(track.scene_ids.clone());
        let children = refs
            .iter()
            .map(|reference| explain_evidence(validated, observations, reference, visiting))
            .collect::<Result<Vec<_>, _>>()?;
        ExplanationNode {
            id: track.id.clone(),
            kind: "ocr_track".to_string(),
            label: format!("{} · {}", track.role, track.text),
            detail: Some(format!("media evidence revision {}", batch.revision)),
            score: None,
            producer: Some(ProducerV1 {
                name: track.provenance.processor.clone(),
                revision: track.provenance.processor_version.clone(),
            }),
            correlation_group: Some(format!("ocr-track:{}", track.id)),
            children,
        }
    } else if validated.evidence_ids.contains(id) {
        ExplanationNode {
            id: id.to_string(),
            kind: "evidence".to_string(),
            label: id.to_string(),
            detail: None,
            score: None,
            producer: None,
            correlation_group: None,
            children: Vec::new(),
        }
    } else {
        return Err(BeliefError::UnknownEvidenceReference {
            observation: "explanation".to_string(),
            reference: id.to_string(),
        });
    };
    visiting.remove(id);
    Ok(node)
}

pub fn find_claim<'a>(claims: &'a [Claim], id: &str) -> Result<&'a Claim, BeliefError> {
    claims
        .iter()
        .find(|claim| claim.id == id)
        .ok_or_else(|| BeliefError::ClaimNotFound(id.to_string()))
}

fn claim_id(subject: &str, predicate: &str, object: &str) -> String {
    format!("claim:{subject}:{predicate}:{object}")
        .replace(' ', "-")
        .to_lowercase()
}
