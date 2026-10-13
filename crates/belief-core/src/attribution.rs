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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    Endorses,
    Disputes,
    Undetermined,
}

impl Stance {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "endorses" => Some(Self::Endorses),
            "disputes" => Some(Self::Disputes),
            "undetermined" => Some(Self::Undetermined),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Endorses => "endorses",
            Self::Disputes => "disputes",
            Self::Undetermined => "undetermined",
        }
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
