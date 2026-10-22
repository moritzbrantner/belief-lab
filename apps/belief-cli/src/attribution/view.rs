//! The per-person view derived from admitted observations and reviewed assessments
//! (Decisions 1.4.25, 1.4.27, 1.4.29 and 1.4.31). Nothing here picks a winner between opposing
//! observations or turns elapsed time into evidence.
use super::*;

pub(super) struct Assessment {
    pub(super) stance: Stance,
    pub(super) confidence_person: Option<AssessedScore>,
    pub(super) observation_ids: Vec<String>,
    pub(super) reviewed_conflicts: bool,
    pub(super) rationale: Option<String>,
    pub(super) as_of: Option<String>,
}

/// The most recent dated attributed stance. It is history, not a claim about the present.
#[derive(Debug, Serialize)]
pub struct LastDocumentedStance {
    pub stance: String,
    pub observation_ids: Vec<String>,
    pub expressed_at: Vec<String>,
    /// Attributed observations without an expression date, which could be later or earlier.
    pub undated_observation_ids: Vec<String>,
}

/// A reviewable estimate of the present stance; `unknown` unless an investigator supplied one.
#[derive(Debug, Serialize)]
pub struct CurrentStance {
    /// `unknown` or `estimated`.
    pub status: &'static str,
    pub stance: Option<String>,
    pub confidence_person: Option<AssessedScore>,
    pub rationale: Option<String>,
    pub as_of: Option<String>,
    pub observation_ids: Vec<String>,
}

pub(super) struct Derived {
    pub(super) status: &'static str,
    pub(super) stance: Option<String>,
    pub(super) confidence_person: Option<AssessedScore>,
    pub(super) supporting_observation_ids: Vec<String>,
    pub(super) conflicting_observation_ids: Vec<String>,
    pub(super) last_documented: Option<LastDocumentedStance>,
    pub(super) current_stance: CurrentStance,
}

pub(super) fn derive(
    person_id: &str,
    observations: &[ObservationReport],
    markers: &BTreeMap<String, ObservationMarker>,
    documented: Option<Assessment>,
    current: Option<Assessment>,
) -> Result<Derived, Box<dyn Error>> {
    let attributed = observations
        .iter()
        .filter_map(|observation| {
            let marker = &markers[&observation.id];
            marker
                .stance
                .is_attributed()
                .then_some((observation, marker))
        })
        .collect::<Vec<_>>();

    let mut conflicting = BTreeSet::new();
    let mut opposed_over_time = false;
    for (index, (left, left_marker)) in attributed.iter().enumerate() {
        for (right, right_marker) in &attributed[index + 1..] {
            if !left_marker.stance.opposes(right_marker.stance) {
                continue;
            }
            match (left_marker.expressed_at, right_marker.expressed_at) {
                (Some(a), Some(b)) if !a.may_overlap(b) => opposed_over_time = true,
                // Unknown or overlapping dates: the same time and context cannot be ruled out.
                _ => {
                    conflicting.insert(left.id.clone());
                    conflicting.insert(right.id.clone());
                }
            }
        }
    }
    for assessment in documented.iter().chain(current.iter()) {
        if !conflicting.is_empty()
            && assessment.confidence_person.is_some()
            && !assessment.reviewed_conflicts
        {
            return Err(invalid(format!(
                "person {person_id} has conflicting evidence; numerical assessment requires reviewedConflicts=true"
            )));
        }
    }

    let status = if documented.is_some() {
        "assessed"
    } else if attributed.is_empty() {
        "no_stance_evidence"
    } else if !conflicting.is_empty() {
        "conflicted"
    } else if opposed_over_time {
        "differs_over_time"
    } else {
        "unassessed"
    };
    let (stance, confidence_person, mut supporting_observation_ids) = match documented {
        Some(assessment) => (
            Some(assessment.stance.as_str().to_owned()),
            assessment.confidence_person,
            assessment.observation_ids,
        ),
        None => (None, None, Vec::new()),
    };
    supporting_observation_ids.sort();

    let current_stance = match current {
        Some(assessment) => {
            let mut observation_ids = assessment.observation_ids;
            observation_ids.sort();
            CurrentStance {
                status: "estimated",
                stance: Some(assessment.stance.as_str().to_owned()),
                confidence_person: assessment.confidence_person,
                rationale: assessment.rationale,
                as_of: assessment.as_of,
                observation_ids,
            }
        }
        None => CurrentStance {
            status: "unknown",
            stance: None,
            confidence_person: None,
            rationale: None,
            as_of: None,
            observation_ids: Vec::new(),
        },
    };

    Ok(Derived {
        status,
        stance,
        confidence_person,
        supporting_observation_ids,
        conflicting_observation_ids: conflicting.into_iter().collect(),
        last_documented: last_documented(&attributed),
        current_stance,
    })
}

fn last_documented(
    attributed: &[(&ObservationReport, &ObservationMarker)],
) -> Option<LastDocumentedStance> {
    let dated = attributed
        .iter()
        .filter_map(|(observation, marker)| {
            Some((*observation, marker.stance, marker.expressed_at?))
        })
        .collect::<Vec<_>>();
    let latest = dated
        .iter()
        .filter(|(_, _, date)| {
            !dated
                .iter()
                .any(|(_, _, other)| date.certainly_before(*other))
        })
        .collect::<Vec<_>>();
    let (_, stance, _) = latest.first()?;
    if latest.iter().any(|(_, other, _)| other != stance) {
        // Differing stances at the latest date are a conflict, not one last documented stance.
        return None;
    }
    let mut expressed_at = latest
        .iter()
        .filter_map(|(observation, _, _)| observation.expressed_at.clone())
        .collect::<Vec<_>>();
    expressed_at.sort();
    expressed_at.dedup();
    Some(LastDocumentedStance {
        stance: stance.as_str().to_owned(),
        observation_ids: latest
            .iter()
            .map(|(observation, _, _)| observation.id.clone())
            .collect(),
        expressed_at,
        undated_observation_ids: attributed
            .iter()
            .filter(|(_, marker)| marker.expressed_at.is_none())
            .map(|(observation, _)| observation.id.clone())
            .collect(),
    })
}
