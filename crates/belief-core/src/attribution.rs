//! Person-belief attribution semantics. None of these scores is proposition-level soft truth.

use crate::ModelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionKind {
    ExplicitEndorsement,
    ReportedPosition,
    InferredBelief,
    QuotationOrCriticism,
}

impl AttributionKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "explicit_endorsement" => Some(Self::ExplicitEndorsement),
            "reported_position" => Some(Self::ReportedPosition),
            "inferred_belief" => Some(Self::InferredBelief),
            "quotation_or_criticism" => Some(Self::QuotationOrCriticism),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitEndorsement => "explicit_endorsement",
            Self::ReportedPosition => "reported_position",
            Self::InferredBelief => "inferred_belief",
            Self::QuotationOrCriticism => "quotation_or_criticism",
        }
    }
}

/// What one source-grounded observation says about a person's stance toward P (Decision 1.4.29).
/// "No evidence of a stance" is not an observation; it is what remains when none of these exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    /// An affirmative belief in P.
    Endorses,
    /// An affirmative belief in not-P; criticism or quotation is not this.
    EndorsesNegation,
    /// An expressed decision to withhold a settled position about P.
    SuspendsJudgment,
    /// The passage bears no stance of this person toward P (for example a quotation).
    NoStance,
}

impl Stance {
    /// Parses current names plus the version-1/2 aliases `disputes` and `undetermined`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "endorses" => Some(Self::Endorses),
            "endorses_negation" | "disputes" => Some(Self::EndorsesNegation),
            "suspends_judgment" => Some(Self::SuspendsJudgment),
            "no_stance" | "undetermined" => Some(Self::NoStance),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Endorses => "endorses",
            Self::EndorsesNegation => "endorses_negation",
            Self::SuspendsJudgment => "suspends_judgment",
            Self::NoStance => "no_stance",
        }
    }

    /// True for the three stances a person can be attributed.
    pub fn is_attributed(self) -> bool {
        self != Self::NoStance
    }

    /// Endorsing P and endorsing not-P oppose each other; suspension opposes neither.
    pub fn opposes(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Endorses, Self::EndorsesNegation) | (Self::EndorsesNegation, Self::Endorses)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvictionBasis {
    SelfReported,
    Inferred,
}

impl ConvictionBasis {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "self_reported" => Some(Self::SelfReported),
            "inferred" => Some(Self::Inferred),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfReported => "self_reported",
            Self::Inferred => "inferred",
        }
    }
}

fn valid_degree(value: f64) -> Result<f64, ModelError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ModelError::InvalidScore(value));
    }
    Ok(value)
}

/// How strongly a single admitted observation implies the person's belief.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservationConfidence(f64);

impl ObservationConfidence {
    pub fn new(value: f64) -> Result<Self, ModelError> {
        Ok(Self(valid_degree(value)?))
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

/// Explicit, reviewable assessment across cited observations, not a model's soft-truth output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PersonConfidence(f64);

impl PersonConfidence {
    pub fn new(value: f64) -> Result<Self, ModelError> {
        Ok(Self(valid_degree(value)?))
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

/// The person's own certainty in a proposition, when supported by the particular statement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conviction(f64);

impl Conviction {
    pub fn new(value: f64) -> Result<Self, ModelError> {
        Ok(Self(valid_degree(value)?))
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_have_independent_domain_types_and_bounds() {
        assert_eq!(ObservationConfidence::new(0.98).unwrap().value(), 0.98);
        assert_eq!(PersonConfidence::new(0.87).unwrap().value(), 0.87);
        assert_eq!(Conviction::new(0.22).unwrap().value(), 0.22);
        assert!(ObservationConfidence::new(-0.1).is_err());
        assert!(PersonConfidence::new(1.01).is_err());
        assert!(Conviction::new(f64::NAN).is_err());
    }

    #[test]
    fn stances_keep_four_distinctions_and_read_legacy_names() {
        for stance in [
            Stance::Endorses,
            Stance::EndorsesNegation,
            Stance::SuspendsJudgment,
            Stance::NoStance,
        ] {
            assert_eq!(Stance::parse(stance.as_str()), Some(stance));
        }
        assert_eq!(Stance::parse("disputes"), Some(Stance::EndorsesNegation));
        assert_eq!(Stance::parse("undetermined"), Some(Stance::NoStance));
        assert!(Stance::Endorses.opposes(Stance::EndorsesNegation));
        assert!(!Stance::SuspendsJudgment.opposes(Stance::Endorses));
        assert!(!Stance::NoStance.is_attributed());
    }

    #[test]
    fn attribution_types_are_exhaustively_named() {
        for kind in [
            AttributionKind::ExplicitEndorsement,
            AttributionKind::ReportedPosition,
            AttributionKind::InferredBelief,
            AttributionKind::QuotationOrCriticism,
        ] {
            assert_eq!(AttributionKind::parse(kind.as_str()), Some(kind));
        }
    }
}
