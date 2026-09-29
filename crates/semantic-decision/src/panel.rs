//! Descriptive model agreement over one exact authorized target, never source corroboration.
use crate::{AuthorizedDecisionRequest, SemanticDecisionReceipt, SemanticJudgment};
use belief_core::{EvidenceFamilyId, JudgmentId, JudgmentSpecRef, Proposition};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelPanel {
    id: String,
    target: AuthorizedDecisionRequest,
    proposition: Proposition,
    spec: JudgmentSpecRef,
    members: BTreeMap<JudgmentId, SemanticJudgment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelSummary {
    pub members: usize,
    pub supports: usize,
    pub contradicts: usize,
    pub unknown: usize,
    pub disagreement: bool,
}

impl ModelPanel {
    pub fn new(
        id: impl Into<String>,
        target: AuthorizedDecisionRequest,
        proposition: Proposition,
        spec: JudgmentSpecRef,
    ) -> Result<Self, String> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err("panel id must not be empty".into());
        }
        Ok(Self {
            id,
            target,
            proposition,
            spec,
            members: BTreeMap::new(),
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn target(&self) -> &AuthorizedDecisionRequest {
        &self.target
    }
    /// Full request equality is checked by the existing receipt conversion boundary.
    pub fn add(&mut self, id: JudgmentId, receipt: SemanticDecisionReceipt) -> Result<(), String> {
        if self.members.contains_key(&id) {
            return Err(format!("duplicate panel member {id}"));
        }
        let judgment = self
            .target
            .clone()
            .into_three_way_judgment(
                receipt,
                id.clone(),
                self.proposition.clone(),
                self.spec.clone(),
            )
            .map_err(|error| error.to_string())?;
        self.members.insert(id, judgment);
        Ok(())
    }
    pub fn members(&self) -> &BTreeMap<JudgmentId, SemanticJudgment> {
        &self.members
    }
    /// Deterministic and independent of panel/model identity. Equal source families share weight.
    pub fn correlation_group(&self) -> Result<EvidenceFamilyId, belief_core::ModelError> {
        let families = self
            .target
            .request()
            .evidence()
            .iter()
            .map(|e| e.evidence.family.as_str())
            .collect::<BTreeSet<_>>();
        // Length-prefixing makes the representation unambiguous even for arbitrary IDs.
        let key = families
            .iter()
            .map(|s| format!("{}:{s}", s.len()))
            .collect::<String>();
        EvidenceFamilyId::new(format!("panel-source:{key}"))
    }
    pub fn summary(&self) -> PanelSummary {
        use belief_core::JudgmentOutcome;
        let mut summary = PanelSummary {
            members: self.members.len(),
            supports: 0,
            contradicts: 0,
            unknown: 0,
            disagreement: false,
        };
        for member in self.members.values() {
            match member.judgment().outcome() {
                JudgmentOutcome::Supports => summary.supports += 1,
                JudgmentOutcome::Contradicts => summary.contradicts += 1,
                JudgmentOutcome::Unknown => summary.unknown += 1,
            }
        }
        summary.disagreement = summary.supports > 0 && summary.contradicts > 0;
        summary
    }
}
