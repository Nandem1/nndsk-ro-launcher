use std::path::Path;

use crate::models::server::ServerConfig;
use crate::tools::runners::{managed_proton_path, MANAGED_RUNNER_ID};
use crate::utils::{
    inspect_prefix, isolated_prefix_path_for_runner, PrefixLocation, PrefixScope, ResolvedRunner,
    RunnerKind,
};

use super::fingerprint::{
    compute_prefix_fingerprint, managed_proton_prefix_fingerprint_input,
    prefix_fingerprint_from_plan, prefix_owned_graphics_for_dxvk, ExternalRoleMaterial,
    PrefixFingerprint, RunnerLocatorIdentity, RunnerPrefixIdentity, Wow64LayoutFingerprint,
};
use super::managed_identity::managed_proton_artifact_identity;
use super::probe::{is_managed_proton, RunnerProbe};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrefixIdentityStatus {
    V3Verified,
    LegacyV2RunnerMatched,
    Unknown,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeEligibility {
    Eligible,
    Ineligible { reasons: Vec<String> },
    Indeterminate { reasons: Vec<String> },
}

#[derive(Debug, Clone)]
pub(crate) struct PrefixBinding {
    pub(crate) status: PrefixIdentityStatus,
    pub(crate) desired_fingerprint: PrefixFingerprint,
    pub(crate) location: PrefixLocation,
    pub(crate) eligibility: RuntimeEligibility,
}

pub(crate) fn prefix_v3_write_enabled() -> bool {
    prefix_v3_write_enabled_from(std::env::var("RO_LAUNCHER_PREFIX_V3").ok().as_deref())
}

pub(crate) fn prefix_v3_write_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

pub(crate) fn resolve_prefix_binding(
    server: Option<&ServerConfig>,
    resolved: &ResolvedRunner,
    probe: &RunnerProbe,
    wine_7_16: bool,
) -> Result<PrefixBinding, String> {
    let desired = desired_prefix_fingerprint(resolved, probe, wine_7_16)?;
    let base_location = crate::utils::resolve_server_prefix_with_runner(
        server,
        Some(
            std::fs::canonicalize(resolved.runner_path())
                .unwrap_or_else(|_| resolved.runner_path().to_path_buf())
                .to_string_lossy()
                .as_ref(),
        ),
    )?;
    resolve_prefix_binding_for_location(server, resolved, &desired, base_location)
}

pub(crate) fn resolve_prefix_binding_for_managed_descriptor(
    server: Option<&ServerConfig>,
) -> Result<PrefixBinding, String> {
    let desired = compute_prefix_fingerprint(&managed_proton_prefix_fingerprint_input());
    let runner_path = managed_proton_path();
    let base = PrefixLocation {
        path: server
            .map(|server| {
                if server.effective_prefix_mode() == crate::models::server::PrefixMode::Isolated {
                    isolated_prefix_path_for_runner(
                        &server.id,
                        runner_path.to_string_lossy().as_ref(),
                    )
                } else {
                    crate::utils::prefix_path()
                }
            })
            .unwrap_or_else(crate::utils::prefix_path),
        scope: server
            .map(|s| match s.effective_prefix_mode() {
                crate::models::server::PrefixMode::Shared => PrefixScope::Shared,
                crate::models::server::PrefixMode::Isolated => PrefixScope::Isolated,
                crate::models::server::PrefixMode::Custom => PrefixScope::Custom,
            })
            .unwrap_or(PrefixScope::Shared),
        managed: server
            .map(|s| s.effective_prefix_mode() != crate::models::server::PrefixMode::Custom)
            .unwrap_or(true),
        server_id: server.map(|s| s.id.clone()),
    };
    resolve_prefix_binding_for_location(server, &managed_proton_stub(), &desired, base)
}

fn managed_proton_stub() -> ResolvedRunner {
    crate::utils::resolved_managed_proton_descriptor()
}

fn resolve_prefix_binding_for_location(
    _server: Option<&ServerConfig>,
    resolved: &ResolvedRunner,
    desired: &PrefixFingerprint,
    mut location: PrefixLocation,
) -> Result<PrefixBinding, String> {
    if location.scope == PrefixScope::Custom {
        return Ok(PrefixBinding {
            status: PrefixIdentityStatus::Unknown,
            desired_fingerprint: desired.clone(),
            location,
            eligibility: RuntimeEligibility::Indeterminate {
                reasons: vec!["custom-prefix".to_string()],
            },
        });
    }
    if location.scope != PrefixScope::Isolated || !location.managed {
        return Ok(PrefixBinding {
            status: PrefixIdentityStatus::Unknown,
            desired_fingerprint: desired.clone(),
            location,
            eligibility: RuntimeEligibility::Eligible,
        });
    }
    let server_id = location
        .server_id
        .as_deref()
        .ok_or_else(|| "El entorno aislado no tiene serverId".to_string())?;

    if let Some(v3_path) = try_v3_verified(server_id, desired) {
        location.path = v3_path;
        return Ok(PrefixBinding {
            status: PrefixIdentityStatus::V3Verified,
            desired_fingerprint: desired.clone(),
            location,
            eligibility: RuntimeEligibility::Eligible,
        });
    }

    let runner_path = std::fs::canonicalize(resolved.runner_path())
        .unwrap_or_else(|_| resolved.runner_path().to_path_buf());
    let v2_path =
        isolated_prefix_path_for_runner(server_id, runner_path.to_string_lossy().as_ref());
    if try_legacy_v2_match(server_id, resolved, &v2_path) {
        location.path = v2_path;
        return Ok(PrefixBinding {
            status: PrefixIdentityStatus::LegacyV2RunnerMatched,
            desired_fingerprint: desired.clone(),
            location,
            eligibility: RuntimeEligibility::Eligible,
        });
    }

    let v2_exists = Path::new(&v2_path).is_dir();
    if v2_exists && !legacy_topology(resolved, wine_7_16_from_runner(resolved)) {
        location.path = if prefix_v3_write_enabled() {
            crate::utils::isolated_prefix_path_v3(server_id, &desired.digest.hex_digest())
        } else {
            v2_path
        };
        return Ok(PrefixBinding {
            status: PrefixIdentityStatus::Unknown,
            desired_fingerprint: desired.clone(),
            location,
            eligibility: RuntimeEligibility::Eligible,
        });
    }

    if prefix_v3_write_enabled() {
        location.path =
            crate::utils::isolated_prefix_path_v3(server_id, &desired.digest.hex_digest());
    } else {
        location.path = v2_path;
    }
    let root = Path::new(&location.path);
    if root.is_dir()
        && root
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some())
    {
        let health = inspect_prefix(&location.path);
        if health.manifest.is_none() && !health.legacy_marker {
            return Ok(PrefixBinding {
                status: PrefixIdentityStatus::Incompatible,
                desired_fingerprint: desired.clone(),
                location,
                eligibility: RuntimeEligibility::Ineligible {
                    reasons: vec!["nonempty-without-manifest".to_string()],
                },
            });
        }
    }
    Ok(PrefixBinding {
        status: PrefixIdentityStatus::Unknown,
        desired_fingerprint: desired.clone(),
        location,
        eligibility: RuntimeEligibility::Eligible,
    })
}

fn try_v3_verified(server_id: &str, desired: &PrefixFingerprint) -> Option<String> {
    let path = crate::utils::isolated_prefix_path_v3(server_id, &desired.digest.hex_digest());
    let health = inspect_prefix(&path);
    if health.manifest.as_ref().is_some_and(|manifest| {
        manifest.schema_version() == crate::utils::PREFIX_SCHEMA_V3
            && manifest.prefix_fingerprint_digest() == Some(desired.digest.hex_digest().as_str())
            && manifest.server_id() == Some(server_id)
    }) {
        Some(path)
    } else {
        None
    }
}

fn try_legacy_v2_match(server_id: &str, resolved: &ResolvedRunner, v2_path: &str) -> bool {
    let health = inspect_prefix(v2_path);
    if !health.structure_ok {
        return false;
    }
    let Some(manifest) = health.manifest.as_ref() else {
        return false;
    };
    if manifest.schema_version() != crate::utils::PREFIX_SCHEMA_VERSION {
        return false;
    }
    if manifest.server_id() != Some(server_id) {
        return false;
    }
    if !legacy_topology(resolved, resolved.is_wine_7_16()) {
        return false;
    }
    manifest.runner_kind() == resolved.kind_label()
        && crate::utils::manifest_matches_runner(
            manifest.as_v2().expect("schema 2"),
            resolved.kind_label(),
            resolved.runner_path().to_string_lossy().as_ref(),
        )
}

fn legacy_topology(resolved: &ResolvedRunner, wine_7_16: bool) -> bool {
    resolved.is_proton() || wine_7_16 || resolved.kind() == RunnerKind::Wine
}

fn wine_7_16_from_runner(resolved: &ResolvedRunner) -> bool {
    resolved.is_wine_7_16()
}

fn desired_prefix_fingerprint(
    resolved: &ResolvedRunner,
    probe: &RunnerProbe,
    wine_7_16: bool,
) -> Result<PrefixFingerprint, String> {
    let (locator, prefix_identity, wow64) = runner_material(resolved, probe)?;
    let graphics = prefix_owned_graphics_for_dxvk(resolved.kind(), wine_7_16);
    Ok(prefix_fingerprint_from_plan(
        resolved.kind(),
        locator,
        prefix_identity,
        wow64,
        graphics,
    ))
}

fn runner_material(
    resolved: &ResolvedRunner,
    probe: &RunnerProbe,
) -> Result<
    (
        RunnerLocatorIdentity,
        RunnerPrefixIdentity,
        Wow64LayoutFingerprint,
    ),
    String,
> {
    let wow64 = match &probe.capabilities.wow64_layout {
        super::model::CapabilityEvidence::Known { value, .. } => match value {
            super::model::Wow64Layout::OldWow64 => Wow64LayoutFingerprint::OldWow64,
            super::model::Wow64Layout::NewWow64 => Wow64LayoutFingerprint::NewWow64,
            super::model::Wow64Layout::Wine32Only => Wow64LayoutFingerprint::Wine32Only,
        },
        super::model::CapabilityEvidence::Unknown { .. } => Wow64LayoutFingerprint::Unknown,
    };
    if is_managed_proton(resolved) {
        return Ok((
            RunnerLocatorIdentity::Managed {
                artifact_id: MANAGED_RUNNER_ID.to_string(),
            },
            RunnerPrefixIdentity::Managed(managed_proton_artifact_identity()),
            wow64,
        ));
    }
    let runner_path = std::fs::canonicalize(resolved.runner_path())
        .unwrap_or_else(|_| resolved.runner_path().to_path_buf());
    if !runner_path.is_file() {
        return Err("Runner no encontrado".to_string());
    }
    let token =
        super::encode::external_runner_location_token(runner_path.as_os_str().as_encoded_bytes());
    let roles = probe
        .identity
        .observed_material
        .roles
        .iter()
        .map(|material| ExternalRoleMaterial {
            role: role_label(material.role),
            digest: material.digest.clone(),
        })
        .collect();
    Ok((
        RunnerLocatorIdentity::External {
            location_token: token,
        },
        RunnerPrefixIdentity::External(roles),
        wow64,
    ))
}

fn role_label(role: super::model::ObservedMaterialRole) -> String {
    match role {
        super::model::ObservedMaterialRole::Entrypoint => "entrypoint",
        super::model::ObservedMaterialRole::WineServer => "wineserver",
        super::model::ObservedMaterialRole::ProtonInnerWine => "proton-inner-wine",
        super::model::ObservedMaterialRole::ProtonVersion => "proton-version",
        super::model::ObservedMaterialRole::UmuEntrypoint => "umu-entrypoint",
        super::model::ObservedMaterialRole::WineTkgConfig => "wine-tkg-config",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_v3_write_enabled_matches_env_semantics() {
        assert!(prefix_v3_write_enabled_from(None));
        assert!(prefix_v3_write_enabled_from(Some("1")));
        assert!(!prefix_v3_write_enabled_from(Some("0")));
    }

    #[test]
    fn managed_descriptor_binding_uses_golden_proton_fingerprint() {
        let binding = resolve_prefix_binding_for_managed_descriptor(None)
            .expect("managed descriptor binding");
        assert_eq!(
            binding.desired_fingerprint.digest.hex_digest(),
            "a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270"
        );
    }
}
