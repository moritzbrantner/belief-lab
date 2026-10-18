//! Human-confirmed associations between source-local records, separate from identity inference.
//! A confirmation records an investigator's decision; it is not proof of upstream identity truth.
use std::collections::BTreeSet;
use std::fmt;

use belief_core::{EntityId, EvidenceClass, EvidencePurpose, EvidenceRef, InferenceClass};

use crate::{AuthorizationProfile, EvidenceDenial, InferenceDenial, PolicyConfig};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourcePerson {
    repository: String,
    scope_id: String,
    subject_id: EntityId,
}

impl SourcePerson {
    pub fn new(
        repository: impl Into<String>,
        scope_id: impl Into<String>,
        subject_id: EntityId,
    ) -> Result<Self, PersonLinkError> {
        let repository = required(repository.into())?;
        let scope_id = required(scope_id.into())?;
        Ok(Self {
            repository,
            scope_id,
            subject_id,
        })
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }
    pub fn subject_id(&self) -> &EntityId {
        &self.subject_id
    }

    fn matches(&self, evidence: &EvidenceRef) -> bool {
        evidence.subject.as_ref() == Some(&self.subject_id)
            && evidence.provenance.source.repository() == self.repository
            && evidence.provenance.source.scope_id() == self.scope_id
    }
}

/// Exact proposed association and its evidence, including provenance ancestors.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonLinkProposal {
    left: SourcePerson,
    right: SourcePerson,
    evidence: Vec<EvidenceRef>,
}

impl PersonLinkProposal {
    pub fn new(
        left: SourcePerson,
        right: SourcePerson,
        mut evidence: Vec<EvidenceRef>,
    ) -> Result<Self, PersonLinkError> {
        if left == right {
            return Err(PersonLinkError::InvalidRequest(
                "a person link requires two different source-local records",
            ));
        }
        if !evidence.iter().any(|item| left.matches(item))
            || !evidence.iter().any(|item| right.matches(item))
        {
            return Err(PersonLinkError::InvalidRequest(
                "source-scoped support for both person records is required",
            ));
        }
        evidence.sort_by(|a, b| a.id.cmp(&b.id));
        if evidence.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(PersonLinkError::InvalidRequest(
                "duplicate support evidence id",
            ));
        }
        // Complete ancestry makes biometric-derived support visible to authorization too.
        let mut available = BTreeSet::new();
        while available.len() < evidence.len() {
            let before = available.len();
            for item in &evidence {
                if item
                    .provenance
                    .parent_evidence
                    .iter()
                    .all(|id| available.contains(id))
                {
                    available.insert(item.id.clone());
                }
            }
            if before == available.len() {
                return Err(PersonLinkError::InvalidRequest(
                    "support evidence has missing or cyclic provenance ancestors",
                ));
            }
        }
        Ok(Self {
            left,
            right,
            evidence,
        })
    }
    pub fn left(&self) -> &SourcePerson {
        &self.left
    }
    pub fn right(&self) -> &SourcePerson {
        &self.right
    }
    pub fn evidence(&self) -> &[EvidenceRef] {
        &self.evidence
    }
}

/// Explicit user action bound to one exact proposal; settings cannot create this value.
/// The application must obtain actor identity and consent through its trusted interaction boundary.
#[derive(Debug, PartialEq)]
pub struct HumanConfirmation {
    proposal: PersonLinkProposal,
    confirmed_by: String,
    reason: String,
}
impl HumanConfirmation {
    pub fn new(
        proposal: PersonLinkProposal,
        confirmed_by: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, PersonLinkError> {
        Ok(Self {
            proposal,
            confirmed_by: required(confirmed_by.into())?,
            reason: required(reason.into())?,
        })
    }
    pub fn confirmed_by(&self) -> &str {
        &self.confirmed_by
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Historical authorization snapshot. Registry writers must check current policy/evidence
/// validity and conflict/revocation state before adopting or reusing a stored association.
#[derive(Debug, PartialEq)]
pub struct ConfirmedPersonLink {
    confirmation: HumanConfirmation,
    policy: PolicyConfig,
}
impl ConfirmedPersonLink {
    pub fn proposal(&self) -> &PersonLinkProposal {
        &self.confirmation.proposal
    }
    pub fn confirmation(&self) -> &HumanConfirmation {
        &self.confirmation
    }
    pub fn policy(&self) -> &PolicyConfig {
        &self.policy
    }
}

impl PolicyConfig {
    /// Record an explicit non-biometric confirmation under ordinary semantic investigation policy.
    /// This does not authorize automatic identity inference or merge any beliefs/investigations.
    pub fn confirm_person_link(
        &self,
        confirmation: HumanConfirmation,
    ) -> Result<ConfirmedPersonLink, PersonLinkError> {
        if self.profile() == AuthorizationProfile::ObserveOnly {
            return Err(PersonLinkError::InvalidRequest(
                "observe_only cannot confirm person links",
            ));
        }
        self.inference_decision(InferenceClass::Descriptive)
            .map_err(PersonLinkError::PolicyDenied)?;
        for item in confirmation.proposal.evidence() {
            match item.class {
                EvidenceClass::UserAssertion
                | EvidenceClass::Metadata
                | EvidenceClass::Transcript
                | EvidenceClass::Ocr
                | EvidenceClass::NamedEntity => {}
                EvidenceClass::FaceTrackReference
                | EvidenceClass::VoiceTrackReference
                | EvidenceClass::ObjectDetection
                | EvidenceClass::SceneDetection => {
                    return Err(PersonLinkError::UnsupportedEvidence(item.class));
                }
            }
            self.evidence_decision(item, EvidencePurpose::Corroboration)
                .map_err(PersonLinkError::EvidenceDenied)?;
        }
        Ok(ConfirmedPersonLink {
            confirmation,
            policy: self.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonLinkError {
    InvalidRequest(&'static str),
    UnsupportedEvidence(EvidenceClass),
    EvidenceDenied(EvidenceDenial),
    PolicyDenied(InferenceDenial),
}
impl fmt::Display for PersonLinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(reason) => f.write_str(reason),
            Self::UnsupportedEvidence(class) => write!(
                f,
                "{} cannot support a confirmed person link",
                class.as_str()
            ),
            Self::EvidenceDenied(reason) => reason.fmt(f),
            Self::PolicyDenied(reason) => reason.fmt(f),
        }
    }
}
impl std::error::Error for PersonLinkError {}
fn required(value: String) -> Result<String, PersonLinkError> {
    if value.trim().is_empty() {
        Err(PersonLinkError::InvalidRequest(
            "source-person identifiers, confirming investigator and reason must not be empty",
        ))
    } else {
        Ok(value)
    }
}
