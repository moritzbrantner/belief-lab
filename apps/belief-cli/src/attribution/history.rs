//! Ordered investigator-supplied annotation snapshots, replayed through the same comparison rules.
use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HistoryDocument {
    schema: String,
    schema_version: u32,
    revisions: Vec<HistoryRevision>,
}

impl HistoryDocument {
    /// A version-2 history built from already projected revisions.
    pub(super) fn new(revisions: Vec<HistoryRevision>) -> Self {
        Self {
            schema: ATTRIBUTION_SCHEMA.to_owned(),
            schema_version: 2,
            revisions,
        }
    }
}

/// A person whose unchanged assessment of one proposition was explicitly reviewed again.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReassessedPerson {
    pub(super) proposition_id: String,
    pub(super) person_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HistoryRevision {
    reason: String,
    document: AttributionDocument,
    /// Allows explicitly reviewing an unchanged numerical assessment after an observation change.
    #[serde(default)]
    reassessed_person_ids: Vec<String>,
    #[serde(default)]
    supersedes: BTreeMap<String, String>,
}

impl HistoryRevision {
    pub(super) fn new(
        reason: String,
        document: AttributionDocument,
        reassessed_person_ids: Vec<String>,
        supersedes: BTreeMap<String, String>,
    ) -> Self {
        Self {
            reason,
            document,
            reassessed_person_ids,
            supersedes,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HistoricalComparison {
    pub revision: usize,
    pub reason: String,
    pub people: Vec<PersonReport>,
    pub supersedes: BTreeMap<String, String>,
}

pub(super) fn compare_history(
    document: HistoryDocument,
    policy: &PolicyConfig,
    store: &InMemoryBeliefStore,
    exporter: &str,
    batch_revision: &str,
    first_revision: usize,
) -> Result<ComparisonReport, Box<dyn Error>> {
    if document.schema != ATTRIBUTION_SCHEMA || document.schema_version != 2 {
        return Err(invalid("unsupported person-attribution schema/version"));
    }
    if document.revisions.is_empty() || document.revisions.len() > 100 {
        return Err(invalid("history requires between 1 and 100 revisions"));
    }
    let mut proposition = None;
    let mut identities = BTreeMap::new();
    let mut seen_observations = BTreeMap::new();
    let mut previous_observations: BTreeMap<String, ObservationInput> = BTreeMap::new();
    let mut previous_assessments: BTreeMap<String, PersonAssessment> = BTreeMap::new();
    let mut stale = BTreeSet::new();
    let mut reports = Vec::new();
    for (index, revision) in document.revisions.into_iter().enumerate() {
        required("revision.reason", &revision.reason)?;
        let document = revision.document;
        if let Some(expected) = &proposition {
            if expected != &document.proposition {
                return Err(invalid(
                    "a revision cannot silently change the scoped proposition",
                ));
            }
        } else {
            proposition = Some(document.proposition.clone());
        }
        for person in &document.people {
            let identity = (
                person.source_repository.clone(),
                person.scope_id.clone(),
                person.subject_id.clone(),
            );
            if identities
                .insert(person.id.clone(), identity.clone())
                .is_some_and(|previous| previous != identity)
            {
                return Err(invalid(
                    "person id changes source-local identity across revisions",
                ));
            }
        }
        let observations = document
            .observations
            .iter()
            .map(|item| (item.id.clone(), item.clone()))
            .collect::<BTreeMap<_, _>>();
        for (id, observation) in &observations {
            if seen_observations
                .get(id)
                .is_some_and(|previous| previous != observation)
            {
                return Err(invalid("corrected observations require a new id; prior observation records are immutable"));
            }
        }
        for (new, old) in &revision.supersedes {
            let corrected = observations
                .get(new)
                .ok_or_else(|| invalid("superseding observation is missing"))?;
            let original = previous_observations.get(old).ok_or_else(|| {
                invalid("superseded observation is missing from the previous revision")
            })?;
            if new == old
                || observations.contains_key(old)
                || seen_observations.contains_key(new)
                || corrected.person_id != original.person_id
            {
                return Err(invalid("a correction must replace an earlier observation of the same person with a new id"));
            }
        }
        if revision.supersedes.values().collect::<BTreeSet<_>>().len() != revision.supersedes.len()
        {
            return Err(invalid(
                "one original observation cannot have multiple replacements in a revision",
            ));
        }
        seen_observations.extend(
            observations
                .iter()
                .map(|(id, item)| (id.clone(), item.clone())),
        );
        let assessments = document
            .assessments
            .iter()
            .map(|item| (item.person_id.clone(), item.clone()))
            .collect::<BTreeMap<_, _>>();
        let reassessed = revision
            .reassessed_person_ids
            .iter()
            .collect::<BTreeSet<_>>();
        if reassessed.len() != revision.reassessed_person_ids.len()
            || reassessed.iter().any(|id| !assessments.contains_key(*id))
        {
            return Err(invalid(
                "reassessedPersonIds must uniquely name present person assessments",
            ));
        }
        if index > 0 {
            for (id, item) in observations.iter().chain(previous_observations.iter()) {
                if observations.get(id) != previous_observations.get(id) {
                    stale.insert(item.person_id.clone());
                }
            }
            for (id, assessment) in &assessments {
                // An actual changed assessment is a new review. Identical carried-forward values
                // need an explicit marker; a changed reason alone cannot refresh stale support.
                if previous_assessments.get(id) != Some(assessment) || reassessed.contains(id) {
                    stale.remove(id);
                }
            }
        }
        previous_observations = observations;
        previous_assessments = assessments;
        // Validate every supplied snapshot, even if a carried-forward assessment will be stale.
        let report = compare_document(document, policy, store, exporter, batch_revision, &stale)?;
        reports.push((
            report,
            index + first_revision,
            revision.reason,
            revision.supersedes,
        ));
    }
    let (mut report, revision, reason, supersedes) = reports
        .pop()
        .ok_or_else(|| invalid("history has no current revision"))?;
    report.report_version = 2;
    report.revision = Some(revision);
    report.revision_reason = Some(reason);
    report.supersedes = supersedes;
    report.history = reports
        .into_iter()
        .map(
            |(report, revision, reason, supersedes)| HistoricalComparison {
                revision,
                reason,
                people: report.people,
                supersedes,
            },
        )
        .collect();
    Ok(report)
}
