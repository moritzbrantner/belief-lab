//! Portable investigation projects (#35, Decisions 1.1.4–1.1.8): one named investigation with its
//! imported producer evidence batches and its full annotation history. Opening a project replays
//! everything through the same import, policy and attribution rules as a fresh comparison, so a
//! saved file cannot smuggle in state that the current rules would refuse.
use std::collections::BTreeMap;
use std::error::Error;

use belief_store::InMemoryBeliefStore;
use evidence_interchange::ValidatedEvidenceBatch;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attribution::{self, AttributionReport};

pub const PROJECT_SCHEMA: &str = "belief_investigation_project";
pub const PROJECT_VERSION: u32 = 1;
const MAX_BATCHES: usize = 100;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectDocument {
    pub schema: String,
    pub schema_version: u32,
    pub investigation: InvestigationMeta,
    /// Imported producer batches in import order; each stays a separate, receipted import.
    pub evidence_batches: Vec<StoredBatch>,
    /// Person-attribution annotations of any supported version, or null before the first one.
    #[serde(default)]
    pub annotations: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvestigationMeta {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredBatch {
    pub imported_at: String,
    pub batch: Value,
}

#[derive(Debug, Serialize)]
pub struct ProjectReport {
    pub report: &'static str,
    pub report_version: u32,
    pub investigation: InvestigationMeta,
    pub batches: Vec<BatchSummary>,
    /// Absent until the investigation has annotations.
    pub comparison: Option<AttributionReport>,
}

#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub exporter: String,
    pub exporter_revision: String,
    pub revision: String,
    pub imported_at: String,
    pub evidence_ids: Vec<String>,
    /// Records this batch added; the rest were byte-identical to earlier imports.
    pub inserted: usize,
    pub already_present: usize,
}

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    message.into().into()
}

/// A project id is a lowercase slug, safe as a file name and as a storage key.
pub fn valid_project_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.starts_with('-')
}

pub fn open_project(
    json: &str,
    policy: &BTreeMap<String, String>,
) -> Result<ProjectReport, Box<dyn Error>> {
    let project: ProjectDocument = serde_json::from_str(json)?;
    if project.schema != PROJECT_SCHEMA || project.schema_version != PROJECT_VERSION {
        return Err(invalid("unsupported investigation project schema/version"));
    }
    let meta = &project.investigation;
    if !valid_project_id(&meta.id) {
        return Err(invalid(
            "investigation id must be 1-64 lowercase letters, digits or hyphens",
        ));
    }
    if meta.name.trim().is_empty() || meta.name.chars().count() > 200 {
        return Err(invalid("investigation name must be 1-200 characters"));
    }
    if meta.created_at.trim().is_empty() {
        return Err(invalid("investigation createdAt must not be empty"));
    }
    if project.evidence_batches.len() > MAX_BATCHES {
        return Err(invalid(
            "an investigation holds at most 100 evidence batches",
        ));
    }

    let policy = attribution::attribution_policy(policy)?;
    let mut store = InMemoryBeliefStore::default();
    let mut batches = Vec::new();
    for stored in &project.evidence_batches {
        if stored.imported_at.trim().is_empty() {
            return Err(invalid("evidence batch importedAt must not be empty"));
        }
        let batch = ValidatedEvidenceBatch::parse_json(&stored.batch.to_string())?;
        let summary = BatchSummary {
            exporter: batch.exporter_name().to_owned(),
            exporter_revision: batch.exporter_revision().to_owned(),
            revision: batch.revision().to_owned(),
            imported_at: stored.imported_at.clone(),
            evidence_ids: batch
                .evidence()
                .iter()
                .map(|item| item.id.to_string())
                .collect(),
            inserted: 0,
            already_present: 0,
        };
        let outcome = store.insert_authorized_evidence_batch(batch.authorize(&policy)?)?;
        batches.push(BatchSummary {
            inserted: outcome.inserted,
            already_present: outcome.already_present,
            ..summary
        });
    }

    let comparison = match &project.annotations {
        None | Some(Value::Null) => None,
        Some(annotations) => {
            let join = |field: fn(&BatchSummary) -> &str| {
                let mut values = batches.iter().map(field).collect::<Vec<_>>();
                values.dedup();
                values.join(", ")
            };
            Some(attribution::compare_annotations(
                annotations.clone(),
                &policy,
                &store,
                &join(|batch| batch.exporter.as_str()),
                &join(|batch| batch.revision.as_str()),
            )?)
        }
    };
    Ok(ProjectReport {
        report: PROJECT_SCHEMA,
        report_version: PROJECT_VERSION,
        investigation: project.investigation,
        batches,
        comparison,
    })
}
