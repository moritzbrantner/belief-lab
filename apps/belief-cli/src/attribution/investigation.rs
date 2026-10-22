//! Version-3 annotations: one investigation with several scoped propositions and reviewable
//! semantic relationships between them (Decision 1.4.33). Each proposition is compared and
//! replayed on its own; a relationship never creates, moves or supports a person attribution.
use super::history::{HistoryDocument, HistoryRevision, ReassessedPerson};
use super::*;

pub const INVESTIGATION_VERSION: u32 = 3;
const MAX_RELATIONSHIPS: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct InvestigationDocument {
    schema: String,
    schema_version: u32,
    revisions: Vec<InvestigationRevision>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InvestigationRevision {
    reason: String,
    document: Snapshot,
    #[serde(default)]
    reassessed: Vec<ReassessedPerson>,
    /// Corrected observation id -> the earlier observation it replaces.
    #[serde(default)]
    supersedes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    propositions: Vec<ScopedProposition>,
    people: Vec<PersonInput>,
    observations: Vec<ObservationInput>,
    #[serde(default)]
    assessments: Vec<PersonAssessment>,
    #[serde(default)]
    relationships: Vec<RelationshipInput>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RelationshipInput {
    id: String,
    kind: String,
    from_proposition_id: String,
    to_proposition_id: String,
    /// Definitions, time, population and conditions under which the relationship is claimed.
    context: String,
    /// Source-grounded reasoning behind the suggestion.
    reasoning: String,
    #[serde(default)]
    confidence_relation: Option<AssessedScore>,
    suggested_by: String,
    review: Review,
    /// The earlier relationship this one corrects.
    #[serde(default)]
    supersedes: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Review {
    status: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InvestigationReport {
    pub report: &'static str,
    pub report_version: u32,
    pub revision: usize,
    pub revision_reason: String,
    pub policy_profile: String,
    pub evidence_exporter: String,
    pub evidence_batch_revision: String,
    /// One comparison per scoped proposition; people are never pooled across propositions.
    pub propositions: Vec<PropositionComparison>,
    pub relationships: Vec<RelationshipReport>,
    pub relationship_history: Vec<RelationshipSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct PropositionComparison {
    pub proposition: ScopedProposition,
    /// The revision that introduced this proposition.
    pub since_revision: usize,
    pub people: Vec<PersonReport>,
    pub history: Vec<HistoricalComparison>,
    pub supersedes: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RelationshipReport {
    pub id: String,
    /// `equivalence`, `implication` or `partial_overlap`.
    pub kind: String,
    pub from_proposition_id: String,
    pub to_proposition_id: String,
    /// Only implication is directional: from implies to, never the reverse.
    pub directed: bool,
    pub context: String,
    pub reasoning: String,
    pub confidence_relation: Option<AssessedScore>,
    pub suggested_by: String,
    /// `suggested`, `accepted` or `rejected`.
    pub review_status: String,
    pub review_note: Option<String>,
    pub supersedes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RelationshipSnapshot {
    pub revision: usize,
    pub reason: String,
    pub relationships: Vec<RelationshipReport>,
}

pub(super) fn compare_investigation(
    document: InvestigationDocument,
    policy: &PolicyConfig,
    store: &InMemoryBeliefStore,
    exporter: &str,
    batch_revision: &str,
) -> Result<InvestigationReport, Box<dyn Error>> {
    if document.schema != ATTRIBUTION_SCHEMA || document.schema_version != INVESTIGATION_VERSION {
        return Err(invalid("unsupported person-attribution schema/version"));
    }
    if document.revisions.is_empty() || document.revisions.len() > 100 {
        return Err(invalid("history requires between 1 and 100 revisions"));
    }

    // Propositions are immutable once introduced and cannot silently disappear.
    let mut introduced = BTreeMap::<String, (usize, ScopedProposition)>::new();
    let mut observation_owner = BTreeMap::<String, String>::new();
    for (index, revision) in document.revisions.iter().enumerate() {
        let snapshot = &revision.document;
        if snapshot.propositions.is_empty() {
            return Err(invalid("an investigation needs at least one proposition"));
        }
        let mut present = BTreeSet::new();
        for proposition in &snapshot.propositions {
            for (name, value) in [
                ("proposition.id", proposition.id.as_str()),
                ("proposition.text", proposition.text.as_str()),
                ("proposition.scope", proposition.scope.as_str()),
            ] {
                required(name, value)?;
            }
            if !present.insert(proposition.id.clone()) {
                return Err(invalid(format!("duplicate proposition {}", proposition.id)));
            }
            match introduced.get(&proposition.id) {
                Some((_, earlier)) if earlier != proposition => {
                    return Err(invalid(
                        "a revision cannot silently change a scoped proposition; add a new one",
                    ))
                }
                Some(_) => {}
                None => {
                    introduced.insert(proposition.id.clone(), (index, proposition.clone()));
                }
            }
        }
        if introduced.keys().any(|id| !present.contains(id)) {
            return Err(invalid(
                "a proposition cannot be removed from a later revision",
            ));
        }
        for observation in &snapshot.observations {
            let owner = observation.proposition_id.as_deref().ok_or_else(|| {
                invalid(format!(
                    "observation {} needs a propositionId",
                    observation.id
                ))
            })?;
            if !present.contains(owner) {
                return Err(invalid(format!(
                    "observation {} refers to unknown proposition {owner}",
                    observation.id
                )));
            }
            if observation_owner
                .insert(observation.id.clone(), owner.to_owned())
                .is_some_and(|previous| previous != owner)
            {
                return Err(invalid(format!(
                    "observation {} cannot move to another proposition",
                    observation.id
                )));
            }
        }
        for assessment in &snapshot.assessments {
            let owner = assessment.proposition_id.as_deref().ok_or_else(|| {
                invalid(format!(
                    "assessment of {} needs a propositionId",
                    assessment.person_id
                ))
            })?;
            if !present.contains(owner) {
                return Err(invalid(format!(
                    "assessment refers to unknown proposition {owner}"
                )));
            }
        }
        for target in &revision.reassessed {
            if !present.contains(&target.proposition_id) {
                return Err(invalid("reassessed entry refers to an unknown proposition"));
            }
        }
        for (new, old) in &revision.supersedes {
            if observation_owner.get(new) != observation_owner.get(old) {
                return Err(invalid(
                    "a correction must stay with the original observation's proposition",
                ));
            }
        }
    }

    let relationship_history = relationship_history(&document.revisions)?;

    // Replay each proposition from the revision that introduced it, through the shared history rules.
    let mut propositions = Vec::new();
    for (first, proposition) in introduced.values() {
        let revisions = document.revisions[*first..]
            .iter()
            .map(|revision| project(revision, proposition))
            .collect::<Vec<_>>();
        let report = history::compare_history(
            HistoryDocument::new(revisions),
            policy,
            store,
            exporter,
            batch_revision,
            first + 1,
        )?;
        propositions.push(PropositionComparison {
            proposition: report.proposition,
            since_revision: first + 1,
            people: report.people,
            history: report.history,
            supersedes: report.supersedes,
        });
    }

    let last = document
        .revisions
        .last()
        .ok_or_else(|| invalid("history has no current revision"))?;
    let mut history = relationship_history;
    let current = history
        .pop()
        .ok_or_else(|| invalid("history has no current revision"))?;
    Ok(InvestigationReport {
        report: "person_belief_investigation",
        report_version: INVESTIGATION_VERSION,
        revision: document.revisions.len(),
        revision_reason: last.reason.clone(),
        policy_profile: policy.profile().as_str().to_owned(),
        evidence_exporter: exporter.to_owned(),
        evidence_batch_revision: batch_revision.to_owned(),
        propositions,
        relationships: current.relationships,
        relationship_history: history,
    })
}

/// The single-proposition view of one revision: every person, only this proposition's evidence.
fn project(revision: &InvestigationRevision, proposition: &ScopedProposition) -> HistoryRevision {
    let snapshot = &revision.document;
    let owned = |id: &Option<String>| id.as_deref() == Some(proposition.id.as_str());
    let observations = snapshot
        .observations
        .iter()
        .filter(|observation| owned(&observation.proposition_id))
        .cloned()
        .collect::<Vec<_>>();
    let mine = observations
        .iter()
        .map(|observation| observation.id.clone())
        .collect::<BTreeSet<_>>();
    HistoryRevision::new(
        revision.reason.clone(),
        AttributionDocument {
            schema: ATTRIBUTION_SCHEMA.to_owned(),
            schema_version: ATTRIBUTION_VERSION,
            proposition: proposition.clone(),
            people: snapshot.people.clone(),
            observations,
            assessments: snapshot
                .assessments
                .iter()
                .filter(|assessment| owned(&assessment.proposition_id))
                .cloned()
                .collect(),
        },
        revision
            .reassessed
            .iter()
            .filter(|target| target.proposition_id == proposition.id)
            .map(|target| target.person_id.clone())
            .collect(),
        revision
            .supersedes
            .iter()
            .filter(|(new, _)| mine.contains(*new))
            .map(|(new, old)| (new.clone(), old.clone()))
            .collect(),
    )
}

fn relationship_history(
    revisions: &[InvestigationRevision],
) -> Result<Vec<RelationshipSnapshot>, Box<dyn Error>> {
    let mut seen = BTreeMap::<String, RelationshipInput>::new();
    let mut previous = BTreeSet::<String>::new();
    let mut snapshots = Vec::new();
    for (index, revision) in revisions.iter().enumerate() {
        required("revision.reason", &revision.reason)?;
        let snapshot = &revision.document;
        if snapshot.relationships.len() > MAX_RELATIONSHIPS {
            return Err(invalid("too many proposition relationships"));
        }
        let propositions = snapshot
            .propositions
            .iter()
            .map(|proposition| proposition.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut current = BTreeSet::new();
        let mut reports = Vec::new();
        for link in &snapshot.relationships {
            required("relationship.id", &link.id)?;
            required("relationship.context", &link.context)?;
            required("relationship.reasoning", &link.reasoning)?;
            if !current.insert(link.id.clone()) {
                return Err(invalid(format!("duplicate relationship {}", link.id)));
            }
            let directed = match link.kind.as_str() {
                "implication" => true,
                "equivalence" | "partial_overlap" => false,
                other => return Err(invalid(format!("unsupported relationship kind {other}"))),
            };
            if link.from_proposition_id == link.to_proposition_id
                || !propositions.contains(link.from_proposition_id.as_str())
                || !propositions.contains(link.to_proposition_id.as_str())
            {
                return Err(invalid(format!(
                    "relationship {} must join two distinct propositions of this investigation",
                    link.id
                )));
            }
            if !matches!(link.suggested_by.as_str(), "analyst" | "scripted_example") {
                return Err(invalid(
                    "suggestedBy must be analyst or scripted_example; model suggestions need their own receipts",
                ));
            }
            if !matches!(
                link.review.status.as_str(),
                "suggested" | "accepted" | "rejected"
            ) {
                return Err(invalid(
                    "review.status must be suggested, accepted or rejected",
                ));
            }
            if let Some(score) = &link.confidence_relation {
                ObservationConfidence::new(score.value)?;
                score.validate()?;
            }
            if let Some(earlier) = seen.get(&link.id) {
                // Reviewing an existing link may change only its review; a changed claim is a
                // correction under a new id.
                let mut same = link.clone();
                same.review = earlier.review.clone();
                if &same != earlier {
                    return Err(invalid(
                        "corrected relationships require a new id; prior relationship records are immutable",
                    ));
                }
            }
            if let Some(old) = &link.supersedes {
                if old == &link.id
                    || (!seen.contains_key(&link.id) && !previous.contains(old))
                    || snapshot.relationships.iter().any(|item| &item.id == old)
                {
                    return Err(invalid(
                        "a relationship correction must replace one from the previous revision",
                    ));
                }
            }
            reports.push(RelationshipReport {
                id: link.id.clone(),
                kind: link.kind.clone(),
                from_proposition_id: link.from_proposition_id.clone(),
                to_proposition_id: link.to_proposition_id.clone(),
                directed,
                context: link.context.clone(),
                reasoning: link.reasoning.clone(),
                confidence_relation: link.confidence_relation.clone(),
                suggested_by: link.suggested_by.clone(),
                review_status: link.review.status.clone(),
                review_note: link.review.note.clone(),
                supersedes: link.supersedes.clone(),
            });
        }
        for link in &snapshot.relationships {
            seen.insert(link.id.clone(), link.clone());
        }
        previous = current;
        reports.sort_by(|a, b| a.id.cmp(&b.id));
        snapshots.push(RelationshipSnapshot {
            revision: index + 1,
            reason: revision.reason.clone(),
            relationships: reports,
        });
    }
    Ok(snapshots)
}
