use std::collections::{BTreeSet, HashMap};

use corpus_core::{AssetId, SegmentId, SourceId};

use crate::model::*;

#[derive(Debug, Clone)]
pub struct ValidatedBundle {
    pub(crate) bundle: BeliefEvidenceBundleV1,
    source_ids: HashMap<String, SourceId>,
    span_ids: HashMap<String, SegmentId>,
    video_ids: HashMap<String, AssetId>,
    pub(crate) evidence_ids: BTreeSet<String>,
}

impl ValidatedBundle {
    pub fn bundle(&self) -> &BeliefEvidenceBundleV1 {
        &self.bundle
    }

    pub fn source_count(&self) -> usize {
        self.source_ids.len()
    }

    pub fn span_count(&self) -> usize {
        self.span_ids.len()
    }

    pub fn video_count(&self) -> usize {
        self.video_ids.len()
    }

    pub fn evidence_count(&self) -> usize {
        self.evidence_ids.len()
    }
}

pub fn validate_bundle(bundle: BeliefEvidenceBundleV1) -> Result<ValidatedBundle, BeliefError> {
    if bundle.schema != BELIEF_EVIDENCE_SCHEMA
        || bundle.schema_version != BELIEF_EVIDENCE_VERSION_V1
    {
        return Err(BeliefError::UnsupportedBundle {
            schema: bundle.schema.clone(),
            version: bundle.schema_version,
        });
    }
    validate_producer(&bundle.source_spans.producer)?;
    if bundle.source_spans.schema != SOURCE_SPAN_INTERCHANGE_SCHEMA
        || bundle.source_spans.schema_version != SOURCE_SPAN_INTERCHANGE_VERSION_V1
    {
        return Err(BeliefError::UnsupportedSourceSpans {
            schema: bundle.source_spans.schema.clone(),
            version: bundle.source_spans.schema_version,
        });
    }

    let mut evidence_ids = BTreeSet::new();
    let mut source_ids = HashMap::new();
    for source in &bundle.source_spans.sources {
        validate_non_blank("source id", &source.id)?;
        validate_non_blank("source revision", &source.revision)?;
        let typed = SourceId::new(source.id.clone())
            .map_err(|error| BeliefError::CorpusIdentifier(error.to_string()))?;
        source_ids.insert(source.id.clone(), typed);
        add_evidence_id(&mut evidence_ids, &source.id)?;
    }

    let mut span_ids = HashMap::new();
    for span in &bundle.source_spans.spans {
        validate_non_blank("span id", &span.id)?;
        if !source_ids.contains_key(&span.source_id) {
            return Err(BeliefError::UnknownSource {
                span: span.id.clone(),
                source_id: span.source_id.clone(),
            });
        }
        validate_locator(span)?;
        let typed = SegmentId::new(span.id.clone())
            .map_err(|error| BeliefError::CorpusIdentifier(error.to_string()))?;
        span_ids.insert(span.id.clone(), typed);
        add_evidence_id(&mut evidence_ids, &span.id)?;
    }

    let mut video_ids = HashMap::new();
    for media in &bundle.media_evidence {
        if media.schema != MEDIA_EVIDENCE_SCHEMA
            || media.schema_version != MEDIA_EVIDENCE_VERSION_V1
        {
            return Err(BeliefError::UnsupportedMediaEvidence {
                schema: media.schema.clone(),
                version: media.schema_version,
            });
        }
        validate_producer(&media.producer)?;
        validate_non_blank("media evidence revision", &media.revision)?;
        let typed = AssetId::new(media.video.id.clone())
            .map_err(|error| BeliefError::CorpusIdentifier(error.to_string()))?;
        video_ids.insert(media.video.id.clone(), typed);
        add_evidence_id(&mut evidence_ids, &media.video.id)?;
        for scene in &media.scenes {
            validate_range(&scene.id, scene.start_seconds, scene.end_seconds)?;
            add_evidence_id(&mut evidence_ids, &scene.id)?;
        }
        for observation in &media.ocr_observations {
            if let Some(score) = observation.confidence {
                validate_score(&observation.id, score)?;
            }
            add_evidence_id(&mut evidence_ids, &observation.id)?;
        }
        for track in &media.ocr_tracks {
            if let (Some(start), Some(end)) = (track.start_seconds, track.end_seconds) {
                validate_range(&track.id, start, end)?;
            }
            add_evidence_id(&mut evidence_ids, &track.id)?;
        }
        if let Some(sponsorblock) = &media.sponsorblock {
            add_evidence_id(&mut evidence_ids, &sponsorblock.snapshot_id)?;
            for segment in &sponsorblock.segments {
                validate_range(&segment.uuid, segment.start_seconds, segment.end_seconds)?;
                add_evidence_id(&mut evidence_ids, &segment.uuid)?;
            }
        }
    }

    for observation in &bundle.observations {
        validate_non_blank("observation id", &observation.id)?;
        validate_producer(&observation.provenance)?;
        if let Some(score) = &observation.score {
            validate_score(&observation.id, score.value)?;
        }
        add_evidence_id(&mut evidence_ids, &observation.id)?;
    }

    for observation in &bundle.observations {
        for reference in &observation.evidence_refs {
            if !evidence_ids.contains(reference) {
                return Err(BeliefError::UnknownEvidenceReference {
                    observation: observation.id.clone(),
                    reference: reference.clone(),
                });
            }
        }
    }

    Ok(ValidatedBundle {
        bundle,
        source_ids,
        span_ids,
        video_ids,
        evidence_ids,
    })
}

fn validate_non_blank(kind: &str, value: &str) -> Result<(), BeliefError> {
    if value.trim().is_empty() {
        return Err(BeliefError::BlankIdentifier(kind.to_string()));
    }
    Ok(())
}

fn validate_producer(producer: &ProducerV1) -> Result<(), BeliefError> {
    validate_non_blank("producer name", &producer.name)?;
    validate_non_blank("producer revision", &producer.revision)
}

fn add_evidence_id(ids: &mut BTreeSet<String>, id: &str) -> Result<(), BeliefError> {
    if !ids.insert(id.to_string()) {
        return Err(BeliefError::DuplicateEvidence(id.to_string()));
    }
    Ok(())
}

fn validate_score(id: &str, value: f64) -> Result<(), BeliefError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(BeliefError::InvalidScore { id: id.to_string() });
    }
    Ok(())
}

fn validate_range(id: &str, start: f64, end: f64) -> Result<(), BeliefError> {
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end < start {
        return Err(BeliefError::InvalidMediaRange { id: id.to_string() });
    }
    Ok(())
}

fn validate_locator(span: &SourceSpanRecordV1) -> Result<(), BeliefError> {
    match &span.locator {
        SourceLocatorV1::Text {
            byte_start,
            byte_end,
            ..
        } => {
            if byte_end < byte_start {
                return Err(BeliefError::InvalidTimedSpan {
                    span: span.id.clone(),
                });
            }
        }
        SourceLocatorV1::Timed {
            start_seconds,
            end_seconds,
            ..
        } => {
            if let Some(start) = start_seconds {
                if !start.is_finite() || *start < 0.0 {
                    return Err(BeliefError::InvalidTimedSpan {
                        span: span.id.clone(),
                    });
                }
            }
            if let Some(end) = end_seconds {
                if !end.is_finite() || *end < 0.0 {
                    return Err(BeliefError::InvalidTimedSpan {
                        span: span.id.clone(),
                    });
                }
            }
            if let (Some(start), Some(end)) = (start_seconds, end_seconds) {
                if end < start {
                    return Err(BeliefError::InvalidTimedSpan {
                        span: span.id.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}
