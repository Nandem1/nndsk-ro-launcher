use crate::models::dependency::{
    CompatibilityAssessmentIpc, CompatibilityRecommendationIpc, CompatibilityStatus, RuntimeCheck,
    RuntimeCheckSeverity,
};
use crate::tools::prefix::MANAGED_DXVK_COMPONENT;
use crate::tools::server_tools::{
    GepardInspection, GepardRunnerProfile, GEPARD_HONEY_SHA256, GEPARD_SAKURA_SHA256,
};
use crate::utils::ResolvedRunner;

use super::model::RuntimePlan;
use super::probe::{
    is_managed_proton, probe_managed_proton_identity, probe_wine_716_old_wow64_layout, RunnerProbe,
};

pub(crate) const COMPATIBILITY_CATALOG_SCHEMA_VERSION: u32 = 1;

pub(crate) fn runtime_compat_enabled() -> bool {
    runtime_compat_enabled_from(std::env::var("RO_LAUNCHER_RUNTIME_COMPAT").ok().as_deref())
}

pub(crate) fn runtime_compat_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceId(String);

impl EvidenceId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into();
        if valid_stable_id(&value) {
            Ok(Self(value))
        } else {
            Err("invalid-evidence-id")
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_stable_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '/' | '-')
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompatibilityAssessment {
    Validated {
        evidence_id: EvidenceId,
    },
    Experimental {
        evidence_id: EvidenceId,
    },
    Incompatible {
        evidence_id: EvidenceId,
        reason: FailureClass,
    },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureClass {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceProvenance {
    CuratedShipped,
    LocalObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecommendedProfileId {
    ManagedProtonCachyos11,
    Wine716OldWow64ManagedDxvk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecommendationReason {
    ValidatedGepardHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeRecommendation {
    pub profile: RecommendedProfileId,
    pub evidence_id: EvidenceId,
    pub reason: RecommendationReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubjectObservation {
    Absent,
    Unreadable,
    Hash { sha256_lowercase: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompatibilityRuntimeSpec {
    ManagedProtonCachyos11,
    Wine716OldWow64ManagedDxvk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompatibilityRecordV1 {
    pub evidence_id: EvidenceId,
    pub outcome: CompatibilityAssessment,
    pub recorded_at: &'static str,
    pub provenance: EvidenceProvenance,
    pub gepard_sha256: &'static str,
    pub gepard_product_version: &'static str,
    pub gepard_file_version: &'static str,
    pub runtime: CompatibilityRuntimeSpec,
    pub recommendation_profile: RecommendedProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompatibilitySnapshot {
    pub assessment: CompatibilityAssessment,
    pub recommendation: Option<RuntimeRecommendation>,
    pub subject: SubjectObservation,
}

pub(crate) struct AssessedRuntime<'a> {
    pub plan: &'a RuntimePlan,
    pub probe: &'a RunnerProbe,
}

fn honey_evidence_id() -> EvidenceId {
    EvidenceId::new("gepard-26.8.26.1-proton-cachyos-11").expect("evidence id")
}

fn sakura_evidence_id() -> EvidenceId {
    EvidenceId::new("gepard-26.9.3.1-wine-7.16-old-wow64-dxvk-2.6.2").expect("evidence id")
}

pub(crate) fn shipped_compatibility_records() -> &'static [CompatibilityRecordV1] {
    static RECORDS: std::sync::OnceLock<Vec<CompatibilityRecordV1>> = std::sync::OnceLock::new();
    RECORDS.get_or_init(|| {
        vec![
            CompatibilityRecordV1 {
                evidence_id: honey_evidence_id(),
                outcome: CompatibilityAssessment::Validated {
                    evidence_id: honey_evidence_id(),
                },
                recorded_at: "2026-09-16",
                provenance: EvidenceProvenance::CuratedShipped,
                gepard_sha256: GEPARD_HONEY_SHA256,
                gepard_product_version: "3.0",
                gepard_file_version: "26.8.26.1",
                runtime: CompatibilityRuntimeSpec::ManagedProtonCachyos11,
                recommendation_profile: RecommendedProfileId::ManagedProtonCachyos11,
            },
            CompatibilityRecordV1 {
                evidence_id: sakura_evidence_id(),
                outcome: CompatibilityAssessment::Validated {
                    evidence_id: sakura_evidence_id(),
                },
                recorded_at: "2026-09-16",
                provenance: EvidenceProvenance::CuratedShipped,
                gepard_sha256: GEPARD_SAKURA_SHA256,
                gepard_product_version: "3.0",
                gepard_file_version: "26.9.3.1",
                runtime: CompatibilityRuntimeSpec::Wine716OldWow64ManagedDxvk,
                recommendation_profile: RecommendedProfileId::Wine716OldWow64ManagedDxvk,
            },
        ]
    })
}

pub(crate) fn find_shipped_record_by_hash(sha256: &str) -> Option<&'static CompatibilityRecordV1> {
    shipped_compatibility_records()
        .iter()
        .find(|record| record.gepard_sha256.eq_ignore_ascii_case(sha256))
}

pub(crate) fn curated_gepard_runner_for_hash(sha256: &str) -> Option<GepardRunnerProfile> {
    find_shipped_record_by_hash(sha256)
        .map(|record| recommended_profile_to_gepard_runner(record.recommendation_profile))
}

pub(crate) fn recommended_profile_to_gepard_runner(
    profile: RecommendedProfileId,
) -> GepardRunnerProfile {
    match profile {
        RecommendedProfileId::ManagedProtonCachyos11 => GepardRunnerProfile::ModernProton,
        RecommendedProfileId::Wine716OldWow64ManagedDxvk => GepardRunnerProfile::Wine716Legacy,
    }
}

pub(crate) fn recommendation_to_gepard_profile(
    recommendation: &RuntimeRecommendation,
) -> GepardRunnerProfile {
    recommended_profile_to_gepard_runner(recommendation.profile)
}

pub(crate) fn observed_runtime_spec(
    plan: &RuntimePlan,
    probe: &RunnerProbe,
) -> Option<CompatibilityRuntimeSpec> {
    let runner = plan.runner();
    if is_managed_proton(runner.resolved()) {
        if probe_managed_proton_identity(probe) {
            return Some(CompatibilityRuntimeSpec::ManagedProtonCachyos11);
        }
        return None;
    }

    let managed_dxvk =
        plan.graphics().dxvk_provider().managed_dxvk_artifact_id() == Some(MANAGED_DXVK_COMPONENT);

    if probe_wine_716_old_wow64_layout(probe) && managed_dxvk {
        Some(CompatibilityRuntimeSpec::Wine716OldWow64ManagedDxvk)
    } else {
        None
    }
}

pub(crate) fn assess_compatibility(
    subject: &SubjectObservation,
    runtime: Option<&AssessedRuntime<'_>>,
) -> CompatibilitySnapshot {
    let SubjectObservation::Hash { sha256_lowercase } = subject else {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation: None,
            subject: subject.clone(),
        };
    };

    let Some(record) = find_shipped_record_by_hash(sha256_lowercase) else {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation: None,
            subject: subject.clone(),
        };
    };

    let recommendation = Some(RuntimeRecommendation {
        profile: record.recommendation_profile,
        evidence_id: record.evidence_id.clone(),
        reason: RecommendationReason::ValidatedGepardHash,
    });

    if record.provenance != EvidenceProvenance::CuratedShipped {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation,
            subject: subject.clone(),
        };
    }

    let Some(runtime) = runtime else {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation,
            subject: subject.clone(),
        };
    };

    let observed = observed_runtime_spec(runtime.plan, runtime.probe);
    let assessment = match (observed, &record.outcome) {
        (
            Some(spec),
            CompatibilityAssessment::Validated {
                evidence_id: expected_id,
            },
        ) if spec == record.runtime && *expected_id == record.evidence_id => {
            CompatibilityAssessment::Validated {
                evidence_id: record.evidence_id.clone(),
            }
        }
        _ => CompatibilityAssessment::Unknown,
    };

    CompatibilitySnapshot {
        assessment,
        recommendation,
        subject: subject.clone(),
    }
}

pub(crate) fn assess_against_records(
    subject: &SubjectObservation,
    runtime: Option<&AssessedRuntime<'_>>,
    records: &[CompatibilityRecordV1],
) -> CompatibilitySnapshot {
    let SubjectObservation::Hash { sha256_lowercase } = subject else {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation: None,
            subject: subject.clone(),
        };
    };

    let Some(record) = records
        .iter()
        .find(|record| record.gepard_sha256.eq_ignore_ascii_case(sha256_lowercase))
    else {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation: None,
            subject: subject.clone(),
        };
    };

    let recommendation = Some(RuntimeRecommendation {
        profile: record.recommendation_profile,
        evidence_id: record.evidence_id.clone(),
        reason: RecommendationReason::ValidatedGepardHash,
    });

    if record.provenance != EvidenceProvenance::CuratedShipped {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation,
            subject: subject.clone(),
        };
    }

    let Some(runtime) = runtime else {
        return CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Unknown,
            recommendation,
            subject: subject.clone(),
        };
    };

    let observed = observed_runtime_spec(runtime.plan, runtime.probe);
    let assessment = match (observed, &record.outcome) {
        (
            Some(spec),
            CompatibilityAssessment::Validated {
                evidence_id: expected_id,
            },
        ) if spec == record.runtime && *expected_id == record.evidence_id => {
            CompatibilityAssessment::Validated {
                evidence_id: record.evidence_id.clone(),
            }
        }
        _ => CompatibilityAssessment::Unknown,
    };

    CompatibilitySnapshot {
        assessment,
        recommendation,
        subject: subject.clone(),
    }
}

pub(crate) fn compatibility_ipc(snapshot: &CompatibilitySnapshot) -> CompatibilityStatus {
    let (gepard_file_version, gepard_sha256_prefix) = match &snapshot.subject {
        SubjectObservation::Hash { sha256_lowercase } => {
            let record = find_shipped_record_by_hash(sha256_lowercase);
            let prefix = sha256_lowercase.get(..12).map(str::to_string);
            (record.map(|r| r.gepard_file_version.to_string()), prefix)
        }
        _ => (None, None),
    };

    CompatibilityStatus {
        assessment: assessment_ipc(&snapshot.assessment),
        recommendation: snapshot.recommendation.as_ref().map(recommendation_ipc),
        gepard_file_version,
        gepard_sha256_prefix,
    }
}

fn assessment_ipc(assessment: &CompatibilityAssessment) -> CompatibilityAssessmentIpc {
    match assessment {
        CompatibilityAssessment::Validated { evidence_id } => {
            CompatibilityAssessmentIpc::Validated {
                evidence_id: evidence_id.as_str().to_string(),
            }
        }
        CompatibilityAssessment::Experimental { evidence_id } => {
            CompatibilityAssessmentIpc::Experimental {
                evidence_id: evidence_id.as_str().to_string(),
            }
        }
        CompatibilityAssessment::Incompatible {
            evidence_id,
            reason: _,
        } => CompatibilityAssessmentIpc::Incompatible {
            evidence_id: evidence_id.as_str().to_string(),
            reason: "incompatible".to_string(),
        },
        CompatibilityAssessment::Unknown => CompatibilityAssessmentIpc::Unknown,
    }
}

fn recommendation_ipc(recommendation: &RuntimeRecommendation) -> CompatibilityRecommendationIpc {
    CompatibilityRecommendationIpc {
        profile: recommended_profile_ipc(recommendation.profile),
        evidence_id: recommendation.evidence_id.as_str().to_string(),
        reason: "validatedGepardHash".to_string(),
    }
}

fn recommended_profile_ipc(profile: RecommendedProfileId) -> String {
    match profile {
        RecommendedProfileId::ManagedProtonCachyos11 => "managedProtonCachyos11".to_string(),
        RecommendedProfileId::Wine716OldWow64ManagedDxvk => {
            "wine716OldWow64ManagedDxvk".to_string()
        }
    }
}

fn recommendation_profile_label(profile: RecommendedProfileId) -> &'static str {
    match profile {
        RecommendedProfileId::ManagedProtonCachyos11 => "proton-cachyos-11 administrado",
        RecommendedProfileId::Wine716OldWow64ManagedDxvk => "Wine 7.16 old-WoW64 + DXVK 2.6.2",
    }
}

pub(crate) fn gepard_runtime_check(snapshot: &CompatibilitySnapshot) -> Option<RuntimeCheck> {
    match &snapshot.subject {
        SubjectObservation::Absent => None,
        SubjectObservation::Unreadable => Some(RuntimeCheck {
            id: "gepard-runner".to_string(),
            severity: RuntimeCheckSeverity::Warning,
            message: "No se pudo leer gepard.dll · Unknown".to_string(),
            remediation: None,
        }),
        SubjectObservation::Hash { sha256_lowercase } => {
            let record = find_shipped_record_by_hash(sha256_lowercase);
            if record.is_none() {
                let prefix = sha256_lowercase.get(..12).unwrap_or(sha256_lowercase);
                return Some(RuntimeCheck {
                    id: "gepard-runner".to_string(),
                    severity: RuntimeCheckSeverity::Warning,
                    message: format!("Gepard SHA-256 {prefix}… sin record curated · Unknown"),
                    remediation: Some("Conserva un prefix separado al probar runners".to_string()),
                });
            }
            let record = record.expect("record");
            match &snapshot.assessment {
                CompatibilityAssessment::Validated { evidence_id } => {
                    let validated_label = match record.runtime {
                        CompatibilityRuntimeSpec::ManagedProtonCachyos11 => {
                            "Validated proton-cachyos-11"
                        }
                        CompatibilityRuntimeSpec::Wine716OldWow64ManagedDxvk => {
                            "Validated Wine 7.16 old-WoW64 + DXVK 2.6.2"
                        }
                    };
                    Some(RuntimeCheck {
                        id: "gepard-runner".to_string(),
                        severity: RuntimeCheckSeverity::Ok,
                        message: format!(
                            "Gepard {} build {} · {validated_label} · evidence {}",
                            record.gepard_product_version,
                            record.gepard_file_version,
                            evidence_id.as_str()
                        ),
                        remediation: None,
                    })
                }
                _ => {
                    let remediation = match record.recommendation_profile {
                        RecommendedProfileId::ManagedProtonCachyos11 => {
                            "Selecciona el Proton-CachyOS 11 administrado"
                        }
                        RecommendedProfileId::Wine716OldWow64ManagedDxvk => {
                            "Selecciona Wine 7.16 portable old-WoW64; DXVK 2.6.2 conservará Vulkan moderno"
                        }
                    };
                    Some(RuntimeCheck {
                        id: "gepard-runner".to_string(),
                        severity: RuntimeCheckSeverity::Warning,
                        message: format!(
                            "Gepard {} build {} recomienda {} · evidence {}",
                            record.gepard_product_version,
                            record.gepard_file_version,
                            recommendation_profile_label(record.recommendation_profile),
                            record.evidence_id.as_str()
                        ),
                        remediation: Some(remediation.to_string()),
                    })
                }
            }
        }
    }
}

pub(crate) fn gepard_subject_warnings(
    gepard: Option<GepardInspection>,
    compat_enabled: bool,
) -> Vec<String> {
    let Some(gepard) = gepard else {
        return Vec::new();
    };

    if !compat_enabled {
        return legacy_gepard_warnings(&gepard);
    }

    match &gepard.sha256 {
        None => vec!["No se pudo leer gepard.dll; compatibilidad Unknown.".to_string()],
        Some(sha256) => {
            if let Some(record) = find_shipped_record_by_hash(sha256) {
                let prefix = sha256.get(..12).unwrap_or(sha256.as_str());
                vec![format!(
                    "Gepard Shield {} (FileVersion {}, SHA-256 {}…) reconocido; recomendación {}. Validated se confirma en Dependencias si el runtime coincide.",
                    record.gepard_product_version,
                    record.gepard_file_version,
                    prefix,
                    recommendation_profile_label(record.recommendation_profile),
                )]
            } else {
                let prefix = sha256.get(..12).unwrap_or(sha256.as_str());
                vec![format!(
                    "Build de Gepard no validada (SHA-256 {prefix}…); conserva un prefix separado al probar runners."
                )]
            }
        }
    }
}

fn legacy_gepard_warnings(gepard: &GepardInspection) -> Vec<String> {
    if let Some(build) = gepard.build {
        return vec![format!(
            "Gepard Shield {} (FileVersion {}, SHA-256 {}…) reconocido: perfil validado {}.",
            build.product_version,
            build.file_version,
            &build.sha256[..12],
            build.runner.stack_label(),
        )];
    }
    let fingerprint = gepard
        .sha256
        .as_deref()
        .map(|sha256| format!(" (SHA-256 {}…)", &sha256[..12]))
        .unwrap_or_default();
    vec![format!(
        "Build de Gepard no validada{fingerprint}; conserva un prefix separado al probar runners."
    )]
}

pub(crate) fn legacy_gepard_runner_check(
    build: &crate::tools::server_tools::ValidatedGepardBuild,
    resolved: &ResolvedRunner,
) -> RuntimeCheck {
    let compatible = match build.runner {
        GepardRunnerProfile::ModernProton => resolved.is_proton(),
        GepardRunnerProfile::Wine716Legacy => resolved.is_wine_7_16(),
    };
    RuntimeCheck {
        id: "gepard-runner".to_string(),
        severity: if compatible {
            RuntimeCheckSeverity::Ok
        } else {
            RuntimeCheckSeverity::Warning
        },
        message: if compatible {
            format!(
                "Gepard {} build {} · perfil validado {}",
                build.product_version,
                build.file_version,
                build.runner.stack_label()
            )
        } else {
            format!(
                "Gepard {} build {} recomienda el perfil validado {}",
                build.product_version,
                build.file_version,
                build.runner.stack_label()
            )
        },
        remediation: (!compatible).then(|| build.runner.remediation().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::tools::runners::managed_proton_path;
    use crate::tools::runtime::probe::probe_runner;
    use crate::tools::runtime::resolver::{
        profile_from_legacy, resolve_runtime, DgVoodooState, LegacyProfileInput,
        RuntimeResolutionInput,
    };
    use crate::utils::{PrefixLocation, PrefixScope, ResolvedRunner};
    use serde::Deserialize;

    use super::super::fingerprint::{
        compute_prefix_fingerprint, managed_proton_prefix_fingerprint_input,
    };
    use super::super::identity::{PrefixBinding, PrefixIdentityStatus, RuntimeEligibility};
    use super::*;

    fn prefix_binding(location: PrefixLocation) -> PrefixBinding {
        PrefixBinding {
            status: PrefixIdentityStatus::Unknown,
            desired_fingerprint: compute_prefix_fingerprint(
                &managed_proton_prefix_fingerprint_input(),
            ),
            location,
            eligibility: RuntimeEligibility::Eligible,
        }
    }

    fn resolve_plan(resolved: &ResolvedRunner, dgvoodoo: bool, probe: &RunnerProbe) -> RuntimePlan {
        let profile = profile_from_legacy(LegacyProfileInput {
            server_runner: Some(resolved.runner_path().to_string_lossy().as_ref()),
            default_runner: None,
            resolved,
            dgvoodoo: DgVoodooState::verified(dgvoodoo),
        })
        .unwrap();
        let location = PrefixLocation {
            path: "/tmp/compat-prefix".to_string(),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some("test".to_string()),
        };
        resolve_runtime(RuntimeResolutionInput {
            profile,
            resolved,
            prefix: location.clone(),
            prefix_binding: prefix_binding(location),
            probe: probe.clone(),
            dgvoodoo: DgVoodooState::verified(dgvoodoo),
            webview2_required: false,
        })
        .unwrap()
    }

    fn wine_old_wow64_runner(root: &std::path::Path) -> (ResolvedRunner, RunnerProbe) {
        use std::os::unix::fs::PermissionsExt;

        fs::create_dir_all(root.join("lib/wine/i386-unix")).unwrap();
        fs::create_dir_all(root.join("lib/wine/x86_64-unix")).unwrap();
        fs::write(root.join("lib/wine/i386-unix/libwine.so"), b"x").unwrap();
        fs::write(root.join("lib/wine/x86_64-unix/libwine.so"), b"x").unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        let wine_bin = root.join("bin/wine");
        let wineserver = root.join("bin/wineserver");
        fs::write(&wine_bin, "#!/bin/sh\necho 'wine-7.16'\n").unwrap();
        fs::write(&wineserver, "#!/bin/sh\nexit 0\n").unwrap();
        for path in [&wine_bin, &wineserver] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
        let runner = ResolvedRunner::test_wine(wine_bin, wineserver);
        let probe = probe_runner(&runner);
        (runner, probe)
    }

    #[test]
    fn runtime_compat_flag_semantics() {
        assert!(runtime_compat_enabled_from(None));
        assert!(runtime_compat_enabled_from(Some("1")));
        assert!(!runtime_compat_enabled_from(Some("0")));
    }

    #[test]
    fn honey_hash_managed_proton_validated() {
        let proton_path = managed_proton_path();
        let runner = ResolvedRunner::test_proton(
            proton_path.clone(),
            proton_path.parent().unwrap().to_path_buf(),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let probe = probe_runner(&runner);
        let plan = resolve_plan(&runner, false, &probe);
        let subject = SubjectObservation::Hash {
            sha256_lowercase: GEPARD_HONEY_SHA256.to_string(),
        };
        let snapshot = assess_compatibility(
            &subject,
            Some(&AssessedRuntime {
                plan: &plan,
                probe: &probe,
            }),
        );
        assert!(matches!(
            snapshot.assessment,
            CompatibilityAssessment::Validated { .. }
        ));
    }

    #[test]
    fn honey_hash_external_proton_unknown_with_recommendation() {
        let runner = ResolvedRunner::test_proton(
            PathBuf::from("/opt/other/proton"),
            PathBuf::from("/opt/other"),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let probe = probe_runner(&runner);
        let plan = resolve_plan(&runner, false, &probe);
        let subject = SubjectObservation::Hash {
            sha256_lowercase: GEPARD_HONEY_SHA256.to_string(),
        };
        let snapshot = assess_compatibility(
            &subject,
            Some(&AssessedRuntime {
                plan: &plan,
                probe: &probe,
            }),
        );
        assert_eq!(snapshot.assessment, CompatibilityAssessment::Unknown);
        assert!(snapshot.recommendation.is_some());
    }

    #[test]
    fn sakura_hash_wine_old_wow64_managed_dxvk_validated() {
        let dir = std::env::temp_dir().join(format!("ro-compat-sakura-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (runner, probe) = wine_old_wow64_runner(&dir);
        let plan = resolve_plan(&runner, false, &probe);
        let subject = SubjectObservation::Hash {
            sha256_lowercase: GEPARD_SAKURA_SHA256.to_string(),
        };
        let snapshot = assess_compatibility(
            &subject,
            Some(&AssessedRuntime {
                plan: &plan,
                probe: &probe,
            }),
        );
        assert!(matches!(
            snapshot.assessment,
            CompatibilityAssessment::Validated { .. }
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sakura_hash_wine_without_old_wow64_unknown() {
        let dir = std::env::temp_dir().join(format!("ro-compat-wine716-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let wine_bin = dir.join("bin/wine");
        fs::create_dir_all(wine_bin.parent().unwrap()).unwrap();
        fs::write(&wine_bin, b"").unwrap();
        let wineserver = dir.join("bin/wineserver");
        fs::write(&wineserver, b"").unwrap();
        let runner = ResolvedRunner::test_wine(wine_bin, wineserver);
        let probe = probe_runner(&runner);
        let plan = resolve_plan(&runner, false, &probe);
        let subject = SubjectObservation::Hash {
            sha256_lowercase: GEPARD_SAKURA_SHA256.to_string(),
        };
        let snapshot = assess_compatibility(
            &subject,
            Some(&AssessedRuntime {
                plan: &plan,
                probe: &probe,
            }),
        );
        assert_eq!(snapshot.assessment, CompatibilityAssessment::Unknown);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_hash_no_recommendation() {
        let subject = SubjectObservation::Hash {
            sha256_lowercase: "db4653ddf6aea88a502f10e200a300a05e8e4d65e7cfe65eb5b2ff0779e2e4f0"
                .to_string(),
        };
        let snapshot = assess_compatibility(&subject, None);
        assert_eq!(snapshot.assessment, CompatibilityAssessment::Unknown);
        assert!(snapshot.recommendation.is_none());
    }

    #[test]
    fn honey_hash_without_runtime_unknown_with_recommendation() {
        let subject = SubjectObservation::Hash {
            sha256_lowercase: GEPARD_HONEY_SHA256.to_string(),
        };
        let snapshot = assess_compatibility(&subject, None);
        assert_eq!(snapshot.assessment, CompatibilityAssessment::Unknown);
        assert!(snapshot.recommendation.is_some());
    }

    #[test]
    fn local_observation_record_never_validates() {
        let record = CompatibilityRecordV1 {
            evidence_id: honey_evidence_id(),
            outcome: CompatibilityAssessment::Validated {
                evidence_id: honey_evidence_id(),
            },
            recorded_at: "2026-09-16",
            provenance: EvidenceProvenance::LocalObservation,
            gepard_sha256: GEPARD_HONEY_SHA256,
            gepard_product_version: "3.0",
            gepard_file_version: "26.8.26.1",
            runtime: CompatibilityRuntimeSpec::ManagedProtonCachyos11,
            recommendation_profile: RecommendedProfileId::ManagedProtonCachyos11,
        };
        let subject = SubjectObservation::Hash {
            sha256_lowercase: GEPARD_HONEY_SHA256.to_string(),
        };
        let proton_path = managed_proton_path();
        let runner = ResolvedRunner::test_proton(
            proton_path.clone(),
            proton_path.parent().unwrap().to_path_buf(),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let probe = probe_runner(&runner);
        let plan = resolve_plan(&runner, false, &probe);
        let snapshot = assess_against_records(
            &subject,
            Some(&AssessedRuntime {
                plan: &plan,
                probe: &probe,
            }),
            &[record],
        );
        assert_eq!(snapshot.assessment, CompatibilityAssessment::Unknown);
    }

    #[test]
    fn golden_catalog_matches_shipped_records() {
        #[derive(Deserialize)]
        struct Fixture {
            #[serde(rename = "schemaVersion")]
            schema_version: u32,
            records: Vec<FixtureRecord>,
        }
        #[derive(Deserialize)]
        struct FixtureRecord {
            #[serde(rename = "evidenceId")]
            evidence_id: String,
            #[serde(rename = "gepardSha256")]
            gepard_sha256: String,
            #[serde(rename = "gepardFileVersion")]
            gepard_file_version: String,
        }

        let raw = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../contract-fixtures/runtime-compatibility-catalog-v1.json"),
        )
        .expect("fixture");
        let fixture: Fixture = serde_json::from_str(&raw).expect("json");
        assert_eq!(fixture.schema_version, COMPATIBILITY_CATALOG_SCHEMA_VERSION);
        let shipped = shipped_compatibility_records();
        assert_eq!(shipped.len(), fixture.records.len());
        for (record, golden) in shipped.iter().zip(fixture.records.iter()) {
            assert_eq!(record.evidence_id.as_str(), golden.evidence_id);
            assert_eq!(record.gepard_sha256, golden.gepard_sha256);
            assert_eq!(record.gepard_file_version, golden.gepard_file_version);
        }
    }

    #[test]
    fn gepard_runtime_check_validated_message() {
        let snapshot = CompatibilitySnapshot {
            assessment: CompatibilityAssessment::Validated {
                evidence_id: honey_evidence_id(),
            },
            recommendation: None,
            subject: SubjectObservation::Hash {
                sha256_lowercase: GEPARD_HONEY_SHA256.to_string(),
            },
        };
        let check = gepard_runtime_check(&snapshot).expect("check");
        assert_eq!(check.severity, RuntimeCheckSeverity::Ok);
        assert!(check.message.contains("gepard-26.8.26.1-proton-cachyos-11"));
    }
}
