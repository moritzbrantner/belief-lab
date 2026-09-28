use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use belief_core::{EvidenceClass, EvidencePurpose, EvidenceRef, InferenceClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthorizationProfile {
    ObserveOnly,
    SemanticResearch,
    MultimodalResearch,
}

impl AuthorizationProfile {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "observe_only" => Some(Self::ObserveOnly),
            "semantic_research" => Some(Self::SemanticResearch),
            "multimodal_research" => Some(Self::MultimodalResearch),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ObserveOnly => "observe_only",
            Self::SemanticResearch => "semantic_research",
            Self::MultimodalResearch => "multimodal_research",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BiometricEvidenceMode {
    Off,
    ReferenceOnly,
}

impl BiometricEvidenceMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "reference_only" => Some(Self::ReferenceOnly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityResolutionMode {
    Disabled,
    CorpusLocal,
    RealWorld,
}

impl IdentityResolutionMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "disabled" => Some(Self::Disabled),
            "corpus_local" => Some(Self::CorpusLocal),
            "real_world" => Some(Self::RealWorld),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyConfig {
    profile: AuthorizationProfile,
    pub allowed_evidence: BTreeSet<EvidenceClass>,
    pub allowed_evidence_purposes: BTreeSet<EvidencePurpose>,
    pub allowed_inferences: BTreeSet<InferenceClass>,
    pub biometric_evidence: BiometricEvidenceMode,
    pub identity_resolution: IdentityResolutionMode,
    pub allow_cross_source_join: bool,
    pub require_complete_provenance: bool,
}

impl PolicyConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_pairs(std::env::vars())
    }

    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let values = pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<BTreeMap<_, _>>();

        let profile = parse_value(
            &values,
            "BELIEF_POLICY_PROFILE",
            AuthorizationProfile::ObserveOnly,
            AuthorizationProfile::parse,
        )?;
        let maximum = ProfileMaximum::for_profile(profile);

        let allowed_evidence = parse_allowlist(
            &values,
            "BELIEF_ALLOWED_EVIDENCE",
            &maximum.evidence,
            EvidenceClass::parse,
            profile,
        )?;
        let allowed_evidence_purposes = parse_allowlist(
            &values,
            "BELIEF_ALLOWED_EVIDENCE_PURPOSES",
            &maximum.evidence_purposes,
            EvidencePurpose::parse,
            profile,
        )?;
        let allowed_inferences = parse_allowlist(
            &values,
            "BELIEF_ALLOWED_INFERENCES",
            &maximum.inferences,
            InferenceClass::parse,
            profile,
        )?;

        let biometric_evidence = parse_value(
            &values,
            "BELIEF_BIOMETRIC_EVIDENCE",
            BiometricEvidenceMode::Off,
            BiometricEvidenceMode::parse,
        )?;
        if biometric_evidence > maximum.biometric_evidence {
            return Err(ConfigError::CapabilityExceedsProfile {
                key: "BELIEF_BIOMETRIC_EVIDENCE".into(),
                value: values
                    .get("BELIEF_BIOMETRIC_EVIDENCE")
                    .cloned()
                    .unwrap_or_default(),
                profile,
            });
        }

        let identity_resolution = parse_value(
            &values,
            "BELIEF_IDENTITY_RESOLUTION",
            IdentityResolutionMode::Disabled,
            IdentityResolutionMode::parse,
        )?;
        if identity_resolution > maximum.identity_resolution {
            return Err(ConfigError::CapabilityExceedsProfile {
                key: "BELIEF_IDENTITY_RESOLUTION".into(),
                value: values
                    .get("BELIEF_IDENTITY_RESOLUTION")
                    .cloned()
                    .unwrap_or_default(),
                profile,
            });
        }

        let allow_cross_source_join = parse_bool(&values, "BELIEF_ALLOW_CROSS_SOURCE_JOIN", false)?;
        if allow_cross_source_join && !maximum.allow_cross_source_join {
            return Err(ConfigError::CapabilityExceedsProfile {
                key: "BELIEF_ALLOW_CROSS_SOURCE_JOIN".into(),
                value: "true".into(),
                profile,
            });
        }

        let require_complete_provenance =
            parse_bool(&values, "BELIEF_REQUIRE_COMPLETE_PROVENANCE", true)?;
        if !require_complete_provenance {
            return Err(ConfigError::UnsafeSetting {
                key: "BELIEF_REQUIRE_COMPLETE_PROVENANCE".into(),
                value: "false".into(),
                reason: "belief-lab does not authorize inference from evidence with incomplete provenance"
                    .into(),
            });
        }

        Ok(Self {
            profile,
            allowed_evidence,
            allowed_evidence_purposes,
            allowed_inferences,
            biometric_evidence,
            identity_resolution,
            allow_cross_source_join,
            require_complete_provenance,
        })
    }

    pub fn profile(&self) -> AuthorizationProfile {
        self.profile
    }

    pub fn allows_evidence(&self, class: EvidenceClass, purpose: EvidencePurpose) -> bool {
        self.evidence_class_decision(class, purpose).is_ok()
    }

    /// Explains why an evidence class may or may not be used for a purpose.
    ///
    /// This is the single owner of evidence authorization; the boolean helpers delegate here.
    pub fn evidence_class_decision(
        &self,
        class: EvidenceClass,
        purpose: EvidencePurpose,
    ) -> Result<(), EvidenceDenial> {
        let maximum = ProfileMaximum::for_profile(self.profile);
        if !maximum.evidence.contains(&class) {
            return Err(EvidenceDenial::ClassOutsideProfile {
                class,
                profile: self.profile,
            });
        }
        if !self.allowed_evidence.contains(&class) {
            return Err(EvidenceDenial::ClassNotAllowlisted(class));
        }
        if !maximum.evidence_purposes.contains(&purpose) {
            return Err(EvidenceDenial::PurposeOutsideProfile {
                purpose,
                profile: self.profile,
            });
        }
        if !self.allowed_evidence_purposes.contains(&purpose) {
            return Err(EvidenceDenial::PurposeNotAllowlisted(purpose));
        }

        if class.is_biometric_reference() {
            if self.biometric_evidence != BiometricEvidenceMode::ReferenceOnly
                || self.biometric_evidence > maximum.biometric_evidence
            {
                return Err(EvidenceDenial::BiometricReferencesDisabled(class));
            }
            if purpose == EvidencePurpose::DirectSupport {
                return Err(EvidenceDenial::BiometricDirectSupport(class));
            }
        }

        Ok(())
    }

    pub fn allows_evidence_ref(&self, evidence: &EvidenceRef, purpose: EvidencePurpose) -> bool {
        self.evidence_decision(evidence, purpose).is_ok()
    }

    /// Explains why a concrete evidence reference may or may not be used for a purpose.
    pub fn evidence_decision(
        &self,
        evidence: &EvidenceRef,
        purpose: EvidencePurpose,
    ) -> Result<(), EvidenceDenial> {
        if !self.require_complete_provenance || !evidence.has_complete_provenance() {
            return Err(EvidenceDenial::IncompleteProvenance);
        }
        self.evidence_class_decision(evidence.class, purpose)
    }

    pub fn allows_cross_source_join(&self) -> bool {
        self.allow_cross_source_join
            && ProfileMaximum::for_profile(self.profile).allow_cross_source_join
    }

    pub fn allows_inference(&self, class: InferenceClass) -> bool {
        self.inference_decision(class).is_ok()
    }

    /// Explains why an inference class may or may not be executed.
    ///
    /// Sensitive-trait and real-world identity inference are denied before any profile or
    /// configuration is consulted: no shipped profile can authorize them.
    pub fn inference_decision(&self, class: InferenceClass) -> Result<(), InferenceDenial> {
        if matches!(
            class,
            InferenceClass::SensitiveTrait | InferenceClass::RealWorldIdentity
        ) {
            return Err(InferenceDenial::NeverAuthorized(class));
        }

        let maximum = ProfileMaximum::for_profile(self.profile);
        if !maximum.inferences.contains(&class) {
            return Err(InferenceDenial::ClassOutsideProfile {
                class,
                profile: self.profile,
            });
        }
        if !self.allowed_inferences.contains(&class) {
            return Err(InferenceDenial::ClassNotAllowlisted(class));
        }

        match class {
            InferenceClass::LocalEntityLink
                if self.identity_resolution != IdentityResolutionMode::CorpusLocal =>
            {
                Err(InferenceDenial::IdentityResolutionDisabled)
            }
            InferenceClass::CrossSourceAssociation if !self.allows_cross_source_join() => {
                Err(InferenceDenial::CrossSourceJoinDisabled)
            }
            InferenceClass::LocalEntityLink
            | InferenceClass::CrossSourceAssociation
            | InferenceClass::Descriptive
            | InferenceClass::Preference => Ok(()),
            InferenceClass::SensitiveTrait | InferenceClass::RealWorldIdentity => {
                Err(InferenceDenial::NeverAuthorized(class))
            }
        }
    }
}

/// Environment keys read by [`PolicyConfig::from_pairs`], in documentation order.
pub const POLICY_KEYS: [&str; 8] = [
    "BELIEF_POLICY_PROFILE",
    "BELIEF_ALLOWED_EVIDENCE",
    "BELIEF_ALLOWED_EVIDENCE_PURPOSES",
    "BELIEF_ALLOWED_INFERENCES",
    "BELIEF_BIOMETRIC_EVIDENCE",
    "BELIEF_IDENTITY_RESOLUTION",
    "BELIEF_ALLOW_CROSS_SOURCE_JOIN",
    "BELIEF_REQUIRE_COMPLETE_PROVENANCE",
];

/// Why an evidence reference was not authorized for a purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceDenial {
    IncompleteProvenance,
    ClassOutsideProfile {
        class: EvidenceClass,
        profile: AuthorizationProfile,
    },
    ClassNotAllowlisted(EvidenceClass),
    PurposeOutsideProfile {
        purpose: EvidencePurpose,
        profile: AuthorizationProfile,
    },
    PurposeNotAllowlisted(EvidencePurpose),
    BiometricReferencesDisabled(EvidenceClass),
    BiometricDirectSupport(EvidenceClass),
}

impl fmt::Display for EvidenceDenial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteProvenance => f.write_str(
                "provenance is incomplete (immutable source and producer revisions are required)",
            ),
            Self::ClassOutsideProfile { class, profile } => write!(
                f,
                "evidence class {} is outside profile {}",
                class.as_str(),
                profile.as_str()
            ),
            Self::ClassNotAllowlisted(class) => write!(
                f,
                "evidence class {} is not in BELIEF_ALLOWED_EVIDENCE",
                class.as_str()
            ),
            Self::PurposeOutsideProfile { purpose, profile } => write!(
                f,
                "evidence purpose {} is outside profile {}",
                purpose.as_str(),
                profile.as_str()
            ),
            Self::PurposeNotAllowlisted(purpose) => write!(
                f,
                "evidence purpose {} is not in BELIEF_ALLOWED_EVIDENCE_PURPOSES",
                purpose.as_str()
            ),
            Self::BiometricReferencesDisabled(class) => write!(
                f,
                "{} requires BELIEF_BIOMETRIC_EVIDENCE=reference_only",
                class.as_str()
            ),
            Self::BiometricDirectSupport(class) => {
                write!(f, "{} can never be used as direct_support", class.as_str())
            }
        }
    }
}

/// Why an inference class was not authorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InferenceDenial {
    NeverAuthorized(InferenceClass),
    ClassOutsideProfile {
        class: InferenceClass,
        profile: AuthorizationProfile,
    },
    ClassNotAllowlisted(InferenceClass),
    IdentityResolutionDisabled,
    CrossSourceJoinDisabled,
}

impl fmt::Display for InferenceDenial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NeverAuthorized(class) => write!(
                f,
                "inference class {} is not authorized by any shipped profile",
                class.as_str()
            ),
            Self::ClassOutsideProfile { class, profile } => write!(
                f,
                "inference class {} is outside profile {}",
                class.as_str(),
                profile.as_str()
            ),
            Self::ClassNotAllowlisted(class) => write!(
                f,
                "inference class {} is not in BELIEF_ALLOWED_INFERENCES",
                class.as_str()
            ),
            Self::IdentityResolutionDisabled => {
                f.write_str("local_entity_link requires BELIEF_IDENTITY_RESOLUTION=corpus_local")
            }
            Self::CrossSourceJoinDisabled => {
                f.write_str("cross_source_association requires BELIEF_ALLOW_CROSS_SOURCE_JOIN=true")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidValue {
        key: String,
        value: String,
    },
    CapabilityExceedsProfile {
        key: String,
        value: String,
        profile: AuthorizationProfile,
    },
    UnsafeSetting {
        key: String,
        value: String,
        reason: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { key, value } => {
                write!(f, "invalid value {value:?} for {key}")
            }
            Self::CapabilityExceedsProfile {
                key,
                value,
                profile,
            } => write!(
                f,
                "{key}={value:?} exceeds the maximum capability of profile {}",
                profile.as_str()
            ),
            Self::UnsafeSetting { key, value, reason } => {
                write!(f, "{key}={value:?} is not permitted: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug)]
struct ProfileMaximum {
    evidence: BTreeSet<EvidenceClass>,
    evidence_purposes: BTreeSet<EvidencePurpose>,
    inferences: BTreeSet<InferenceClass>,
    biometric_evidence: BiometricEvidenceMode,
    identity_resolution: IdentityResolutionMode,
    allow_cross_source_join: bool,
}

impl ProfileMaximum {
    fn for_profile(profile: AuthorizationProfile) -> Self {
        use EvidenceClass as E;
        use EvidencePurpose as P;
        use InferenceClass as I;

        match profile {
            AuthorizationProfile::ObserveOnly => Self {
                evidence: set([E::UserAssertion, E::Metadata]),
                evidence_purposes: set([P::DirectSupport]),
                inferences: BTreeSet::new(),
                biometric_evidence: BiometricEvidenceMode::Off,
                identity_resolution: IdentityResolutionMode::Disabled,
                allow_cross_source_join: false,
            },
            AuthorizationProfile::SemanticResearch => Self {
                evidence: set([
                    E::UserAssertion,
                    E::Metadata,
                    E::Transcript,
                    E::Ocr,
                    E::ObjectDetection,
                    E::SceneDetection,
                    E::NamedEntity,
                ]),
                evidence_purposes: set([P::DirectSupport, P::Corroboration]),
                inferences: set([I::Descriptive, I::Preference]),
                biometric_evidence: BiometricEvidenceMode::Off,
                identity_resolution: IdentityResolutionMode::Disabled,
                allow_cross_source_join: false,
            },
            AuthorizationProfile::MultimodalResearch => Self {
                evidence: set([
                    E::UserAssertion,
                    E::Metadata,
                    E::Transcript,
                    E::Ocr,
                    E::ObjectDetection,
                    E::SceneDetection,
                    E::NamedEntity,
                    E::FaceTrackReference,
                    E::VoiceTrackReference,
                ]),
                evidence_purposes: set([P::DirectSupport, P::Corroboration, P::EntityLinking]),
                inferences: set([
                    I::Descriptive,
                    I::Preference,
                    I::LocalEntityLink,
                    I::CrossSourceAssociation,
                ]),
                biometric_evidence: BiometricEvidenceMode::ReferenceOnly,
                identity_resolution: IdentityResolutionMode::CorpusLocal,
                allow_cross_source_join: true,
            },
        }
    }
}

fn set<T: Ord, const N: usize>(values: [T; N]) -> BTreeSet<T> {
    values.into_iter().collect()
}

fn parse_value<T: Copy>(
    values: &BTreeMap<String, String>,
    key: &str,
    default: T,
    parse: fn(&str) -> Option<T>,
) -> Result<T, ConfigError> {
    match values.get(key) {
        Some(value) => parse(value).ok_or_else(|| ConfigError::InvalidValue {
            key: key.into(),
            value: value.clone(),
        }),
        None => Ok(default),
    }
}

fn parse_allowlist<T: Copy + Ord>(
    values: &BTreeMap<String, String>,
    key: &str,
    maximum: &BTreeSet<T>,
    parse: fn(&str) -> Option<T>,
    profile: AuthorizationProfile,
) -> Result<BTreeSet<T>, ConfigError> {
    let Some(raw) = values.get(key) else {
        return Ok(maximum.clone());
    };

    let mut requested = BTreeSet::new();
    for value in raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let parsed = parse(value).ok_or_else(|| ConfigError::InvalidValue {
            key: key.into(),
            value: value.into(),
        })?;
        requested.insert(parsed);
    }

    if !requested.is_subset(maximum) {
        return Err(ConfigError::CapabilityExceedsProfile {
            key: key.into(),
            value: raw.clone(),
            profile,
        });
    }

    Ok(requested)
}

fn parse_bool(
    values: &BTreeMap<String, String>,
    key: &str,
    default: bool,
) -> Result<bool, ConfigError> {
    match values.get(key).map(String::as_str) {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(value) => Err(ConfigError::InvalidValue {
            key: key.into(),
            value: value.into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(values: &[(&str, &str)]) -> Result<PolicyConfig, ConfigError> {
        PolicyConfig::from_pairs(values.iter().copied())
    }

    #[test]
    fn defaults_are_fail_closed() {
        let policy = config(&[]).expect("default policy should load");

        assert_eq!(policy.profile, AuthorizationProfile::ObserveOnly);
        assert!(
            policy.allows_evidence(EvidenceClass::UserAssertion, EvidencePurpose::DirectSupport)
        );
        assert!(!policy.allows_evidence(EvidenceClass::Transcript, EvidencePurpose::DirectSupport));
        assert!(!policy.allows_inference(InferenceClass::Descriptive));
    }

    #[test]
    fn allowlists_can_only_narrow_a_profile() {
        let policy = config(&[
            ("BELIEF_POLICY_PROFILE", "semantic_research"),
            ("BELIEF_ALLOWED_EVIDENCE", "transcript,named_entity"),
            ("BELIEF_ALLOWED_EVIDENCE_PURPOSES", "corroboration"),
            ("BELIEF_ALLOWED_INFERENCES", "descriptive"),
        ])
        .expect("narrowed policy should load");

        assert!(policy.allows_evidence(EvidenceClass::Transcript, EvidencePurpose::Corroboration));
        assert!(!policy.allows_evidence(EvidenceClass::Transcript, EvidencePurpose::DirectSupport));
        assert!(!policy.allows_evidence(EvidenceClass::Ocr, EvidencePurpose::Corroboration));
        assert!(policy.allows_inference(InferenceClass::Descriptive));
        assert!(!policy.allows_inference(InferenceClass::Preference));
    }

    #[test]
    fn biometric_references_require_explicit_opt_in_and_cannot_be_direct_support() {
        let default_multimodal = config(&[("BELIEF_POLICY_PROFILE", "multimodal_research")])
            .expect("multimodal profile should load");
        assert!(!default_multimodal.allows_evidence(
            EvidenceClass::FaceTrackReference,
            EvidencePurpose::EntityLinking
        ));

        let enabled = config(&[
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
        ])
        .expect("reference-only biometric evidence should be permitted");

        assert!(enabled.allows_evidence(
            EvidenceClass::FaceTrackReference,
            EvidencePurpose::EntityLinking
        ));
        assert!(!enabled.allows_evidence(
            EvidenceClass::FaceTrackReference,
            EvidencePurpose::DirectSupport
        ));
    }

    #[test]
    fn local_identity_linking_requires_separate_opt_in() {
        let default_multimodal = config(&[("BELIEF_POLICY_PROFILE", "multimodal_research")])
            .expect("multimodal profile should load");
        assert!(!default_multimodal.allows_inference(InferenceClass::LocalEntityLink));

        let enabled = config(&[
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_IDENTITY_RESOLUTION", "corpus_local"),
        ])
        .expect("corpus-local identity resolution should be permitted");
        assert!(enabled.allows_inference(InferenceClass::LocalEntityLink));
    }

    #[test]
    fn cross_source_association_requires_separate_opt_in() {
        let policy = config(&[
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_ALLOW_CROSS_SOURCE_JOIN", "true"),
        ])
        .expect("cross-source join should be permitted by multimodal profile");
        assert!(policy.allows_inference(InferenceClass::CrossSourceAssociation));
    }

    #[test]
    fn sensitive_and_real_world_identity_inference_are_not_shipped_capabilities() {
        let sensitive = config(&[
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_ALLOWED_INFERENCES", "sensitive_trait"),
        ]);
        assert!(matches!(
            sensitive,
            Err(ConfigError::CapabilityExceedsProfile { .. })
        ));

        let real_world = config(&[
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_IDENTITY_RESOLUTION", "real_world"),
        ]);
        assert!(matches!(
            real_world,
            Err(ConfigError::CapabilityExceedsProfile { .. })
        ));
    }

    #[test]
    fn incomplete_provenance_is_rejected() {
        let policy = config(&[("BELIEF_REQUIRE_COMPLETE_PROVENANCE", "false")]);
        assert!(matches!(policy, Err(ConfigError::UnsafeSetting { .. })));
    }

    #[test]
    fn denials_name_the_policy_reason() {
        let observe = config(&[]).unwrap();
        assert_eq!(
            observe
                .evidence_class_decision(EvidenceClass::Transcript, EvidencePurpose::DirectSupport),
            Err(EvidenceDenial::ClassOutsideProfile {
                class: EvidenceClass::Transcript,
                profile: AuthorizationProfile::ObserveOnly,
            })
        );
        assert_eq!(
            observe.inference_decision(InferenceClass::Preference),
            Err(InferenceDenial::ClassOutsideProfile {
                class: InferenceClass::Preference,
                profile: AuthorizationProfile::ObserveOnly,
            })
        );

        let multimodal = config(&[("BELIEF_POLICY_PROFILE", "multimodal_research")]).unwrap();
        assert_eq!(
            multimodal.evidence_class_decision(
                EvidenceClass::FaceTrackReference,
                EvidencePurpose::Corroboration
            ),
            Err(EvidenceDenial::BiometricReferencesDisabled(
                EvidenceClass::FaceTrackReference
            ))
        );
        assert_eq!(
            multimodal.inference_decision(InferenceClass::LocalEntityLink),
            Err(InferenceDenial::IdentityResolutionDisabled)
        );
        assert_eq!(
            multimodal.inference_decision(InferenceClass::CrossSourceAssociation),
            Err(InferenceDenial::CrossSourceJoinDisabled)
        );

        let biometric = config(&[
            ("BELIEF_POLICY_PROFILE", "multimodal_research"),
            ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
        ])
        .unwrap();
        assert_eq!(
            biometric.evidence_class_decision(
                EvidenceClass::VoiceTrackReference,
                EvidencePurpose::DirectSupport
            ),
            Err(EvidenceDenial::BiometricDirectSupport(
                EvidenceClass::VoiceTrackReference
            ))
        );
    }

    #[test]
    fn sensitive_and_real_world_identity_are_denied_under_every_shipped_profile() {
        let widest = [
            vec![("BELIEF_POLICY_PROFILE", "observe_only")],
            vec![("BELIEF_POLICY_PROFILE", "semantic_research")],
            vec![
                ("BELIEF_POLICY_PROFILE", "multimodal_research"),
                ("BELIEF_BIOMETRIC_EVIDENCE", "reference_only"),
                ("BELIEF_IDENTITY_RESOLUTION", "corpus_local"),
                ("BELIEF_ALLOW_CROSS_SOURCE_JOIN", "true"),
            ],
        ];
        for pairs in widest {
            let policy = config(&pairs).unwrap();
            for class in [
                InferenceClass::SensitiveTrait,
                InferenceClass::RealWorldIdentity,
            ] {
                assert_eq!(
                    policy.inference_decision(class),
                    Err(InferenceDenial::NeverAuthorized(class))
                );
                assert!(!policy.allows_inference(class));
            }
        }
    }
}
