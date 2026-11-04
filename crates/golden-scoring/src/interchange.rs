//! Expresses stage-4 attributions in belief-lab's own contracts: one
//! `belief_evidence_interchange` v2 batch (a reference per attribution, bound to the attributed
//! person's source-local subject, with the claim's media span) and one
//! `belief_person_attributions` v3 investigation, which `belief_cli::attribution::compare`
//! validates and reports like any other.
//!
//! The reporter chain is carried by provenance: a holder's evidence lists the reporter's evidence
//! as its parent. The contracts have no field for the draft's `hedged` flag; it stays in the
//! scoring report only.

use crate::stage4::{PredictedAttribution, Similarities, StageFourDocument};
use crate::GoldenExpectations;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Provenance written into every evidence reference.
#[derive(Debug, Clone)]
pub struct ExportProvenance {
    pub exporter_revision: String,
    pub source_repository: String,
    pub scope_id: String,
    /// Immutable revision of the transcript text (e.g. `sha256:<digest of the document>`).
    pub transcript_revision: String,
    pub producer_name: String,
    pub producer_revision: String,
    pub producer_model: String,
}

/// The golden proposition a claim links to: the most similar one at or above `threshold`,
/// otherwise a proposition of its own (`candidate-<n>`, an unreviewed draft).
pub fn linked_proposition(
    golden: &GoldenExpectations,
    similarities: &Similarities,
    candidate: usize,
    threshold: f64,
) -> Option<String> {
    golden
        .propositions
        .iter()
        .map(|proposition| {
            (
                similarities.get(candidate, &proposition.id),
                &proposition.id,
            )
        })
        .filter(|(similarity, _)| *similarity >= threshold)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id.clone())
}

/// Returns `(evidence batch, investigation)`.
pub fn belief_lab_export(
    golden: &GoldenExpectations,
    document: &StageFourDocument,
    attributions: &[PredictedAttribution],
    similarities: &Similarities,
    threshold: f64,
    provenance: &ExportProvenance,
) -> (Value, Value) {
    let mut evidence = Vec::new();
    let mut observations = Vec::new();
    let mut propositions = BTreeMap::<String, Value>::new();
    // Evidence id of the attribution each candidate's reporters received, by person.
    let mut reporter_evidence = BTreeMap::<(usize, String), String>::new();
    for (index, attribution) in attributions.iter().enumerate() {
        let id = format!("e-{:04}", index + 1);
        if attribution.via.is_none() {
            reporter_evidence.insert(
                (attribution.candidate, attribution.person.clone()),
                id.clone(),
            );
        }
        let parents: Vec<String> = attribution
            .via
            .as_ref()
            .and_then(|via| reporter_evidence.get(&(attribution.candidate, via.clone())))
            .cloned()
            .into_iter()
            .collect();
        let start = (attribution.seconds * 1000.0).round().max(0.0) as i64;
        let end_seconds = document
            .seconds_at(attribution.span.end.saturating_sub(1))
            .unwrap_or(attribution.seconds);
        let end = ((end_seconds * 1000.0).round() as i64).max(start + 1);
        evidence.push(json!({
            "id": id,
            "subject": attribution.person,
            "class": "transcript",
            "familyId": format!("{}:candidate-{}", provenance.scope_id, attribution.candidate),
            "source": {
                "repository": provenance.source_repository,
                "scopeId": provenance.scope_id,
                "recordId": format!("{}:bytes-{}-{}", attribution.segment_id, attribution.span.start, attribution.span.end),
                "revision": provenance.transcript_revision,
            },
            "producer": {
                "name": provenance.producer_name,
                "revision": provenance.producer_revision,
                "model": provenance.producer_model,
            },
            "parentEvidenceIds": parents,
            "mediaSpan": {
                "start": {"pts": start, "timebase": {"num": 1, "den": 1000}},
                "end": {"pts": end, "timebase": {"num": 1, "den": 1000}},
            },
        }));
        let proposition =
            linked_proposition(golden, similarities, attribution.candidate, threshold)
                .unwrap_or_else(|| format!("candidate-{}", attribution.candidate));
        propositions
            .entry(proposition.clone())
            .or_insert_with(|| {
                match golden.propositions.iter().find(|p| p.id == proposition) {
                    Some(golden) => {
                        json!({"id": golden.id, "text": golden.text, "scope": golden.scope})
                    }
                    None => json!({
                        "id": proposition,
                        "text": attribution.canonical_text,
                        "scope": "Unreviewed draft claim candidate",
                    }),
                }
            });
        observations.push(json!({
            "id": format!("o-{:04}", index + 1),
            "propositionId": proposition,
            "personId": attribution.person,
            "evidenceId": id,
            "kind": attribution.kind,
            "stance": attribution.stance,
            "passage": attribution.quote,
            "span": {"start": attribution.span.start, "end": attribution.span.end},
        }));
    }
    let people: Vec<Value> = golden
        .people
        .iter()
        .map(|person| {
            json!({
                "id": person.id,
                "displayName": person.display_name,
                "subjectId": person.id,
                "sourceRepository": provenance.source_repository,
                "scopeId": provenance.scope_id,
            })
        })
        .collect();
    let batch = json!({
        "schema": "belief_evidence_interchange",
        "schemaVersion": 2,
        "exporter": {"name": "belief-lab-golden-media", "revision": provenance.exporter_revision},
        "revision": provenance.transcript_revision,
        "evidence": evidence,
    });
    let investigation = json!({
        "schema": "belief_person_attributions",
        "schemaVersion": 3,
        "revisions": [{
            "reason": "Unreviewed draft attributions from the golden stage-4 run",
            "document": {
                "propositions": propositions.into_values().collect::<Vec<_>>(),
                "people": people,
                "observations": observations,
            },
        }],
    });
    (batch, investigation)
}
