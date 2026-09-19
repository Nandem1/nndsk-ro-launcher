use crate::tools::prefix::MANAGED_DXVK_COMPONENT;
use crate::tools::runners::MANAGED_RUNNER_ID;
use crate::utils::{is_wine_7_16_version, PrefixLocation, ResolvedRunner, RunnerKind};

use super::identity::PrefixBinding;
use super::model::{
    ArtifactId, CapabilityEvidence, ComponentId, ComponentProvenance, DgVoodooOverlayPlan,
    DxvkProvider, GraphicsPlan, GraphicsProfile, LegacyUnpinned, PrefixArchitecture, RunnerPlan,
    RunnerRequest, RunnerSelectionSource, RuntimePlan, RuntimeProfile, SyncPlan, SyncSupport,
    Wow64Layout,
};
use super::probe::{is_managed_proton, paths_match, RunnerProbe};

const RUNNER_DXVK_COMPONENT: &str = "runner/dxvk";
pub(crate) const WINETRICKS_DXVK_RECIPE: &str = "winetricks/dxvk-legacy-unpinned";
const DGVOODOO_COMPONENT: &str = "game-dir/dgvoodoo";
const DGVOODOO_RESOURCE: &str = "bundled/dgvoodoo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DgVoodooState {
    configured: bool,
    wrappers_verified: bool,
}

impl DgVoodooState {
    pub(crate) fn verified(configured: bool) -> Self {
        Self {
            configured,
            wrappers_verified: configured,
        }
    }

    #[cfg(test)]
    fn unverified_configured() -> Self {
        Self {
            configured: true,
            wrappers_verified: false,
        }
    }

    fn ready(self) -> bool {
        self.configured && self.wrappers_verified
    }
}

pub(super) struct LegacyProfileInput<'a> {
    pub(super) server_runner: Option<&'a str>,
    pub(super) default_runner: Option<&'a str>,
    pub(super) resolved: &'a ResolvedRunner,
    pub(super) dgvoodoo: DgVoodooState,
}

pub(crate) fn profile_from_legacy(
    input: LegacyProfileInput<'_>,
) -> Result<RuntimeProfile, RuntimeResolutionError> {
    let selection_source = if nonempty(input.server_runner).is_some() {
        RunnerSelectionSource::ServerOverride
    } else if nonempty(input.default_runner).is_some() {
        RunnerSelectionSource::GlobalSetting
    } else {
        RunnerSelectionSource::ProductDefault
    };
    let runner = if is_managed_proton(input.resolved) {
        RunnerRequest::Managed {
            artifact_id: ArtifactId::new(MANAGED_RUNNER_ID)
                .map_err(|_| RuntimeResolutionError::InvalidKnownId)?,
        }
    } else {
        RunnerRequest::External {
            kind_hint: input.resolved.kind(),
            entrypoint: input.resolved.runner_path().to_path_buf(),
        }
    };
    let graphics = if input.dgvoodoo.ready() {
        GraphicsProfile::DgVoodooDxvk
    } else {
        GraphicsProfile::Dxvk
    };
    Ok(RuntimeProfile::new(runner, selection_source, graphics))
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(super) struct RuntimeResolutionInput<'a> {
    pub(super) profile: RuntimeProfile,
    pub(super) resolved: &'a ResolvedRunner,
    pub(super) prefix: PrefixLocation,
    pub(super) prefix_binding: PrefixBinding,
    pub(super) probe: RunnerProbe,
    pub(super) dgvoodoo: DgVoodooState,
    pub(super) webview2_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeResolutionError {
    InvalidKnownId,
    RunnerRequestMismatch,
    ProbeKindMismatch,
    ProbeProvenanceMismatch,
    CapabilityContradiction,
    DgVoodooNotVerified,
}

impl RuntimeResolutionError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::InvalidKnownId => "invalid-known-id",
            Self::RunnerRequestMismatch => "runner-request-mismatch",
            Self::ProbeKindMismatch => "probe-kind-mismatch",
            Self::ProbeProvenanceMismatch => "probe-provenance-mismatch",
            Self::CapabilityContradiction => "capability-contradiction",
            Self::DgVoodooNotVerified => "dgvoodoo-not-verified",
        }
    }
}

pub(crate) fn resolve_runtime(
    input: RuntimeResolutionInput<'_>,
) -> Result<RuntimePlan, RuntimeResolutionError> {
    validate_runner_request(input.profile.runner(), input.resolved, &input.probe)?;
    validate_capabilities(input.resolved.kind(), &input.probe)?;

    let runner = RunnerPlan {
        resolved: input.resolved.clone(),
        identity: input.probe.identity,
        capabilities: input.probe.capabilities,
        sync: input.probe.sync,
    };
    let dxvk = resolve_dxvk_provider(&runner)?;
    let graphics = match input.profile.graphics() {
        GraphicsProfile::Dxvk => GraphicsPlan::dxvk(dxvk),
        GraphicsProfile::DgVoodooDxvk => {
            if !input.dgvoodoo.ready() {
                return Err(RuntimeResolutionError::DgVoodooNotVerified);
            }
            GraphicsPlan::dgvoodoo_dxvk(
                DgVoodooOverlayPlan {
                    provenance: ComponentProvenance::BundledResource {
                        resource_id: DGVOODOO_RESOURCE.to_string(),
                    },
                    component_id: ComponentId::new(DGVOODOO_COMPONENT)
                        .map_err(|_| RuntimeResolutionError::InvalidKnownId)?,
                },
                dxvk,
            )
        }
    };

    Ok(RuntimePlan {
        runner,
        graphics,
        prefix: input.prefix,
        webview2_required: input.webview2_required,
        prefix_binding: input.prefix_binding,
    })
}

fn validate_runner_request(
    request: &RunnerRequest,
    resolved: &ResolvedRunner,
    probe: &RunnerProbe,
) -> Result<(), RuntimeResolutionError> {
    if probe.identity.kind != resolved.kind() {
        return Err(RuntimeResolutionError::ProbeKindMismatch);
    }
    match request {
        RunnerRequest::Managed { artifact_id } => {
            if artifact_id.as_str() != MANAGED_RUNNER_ID || !is_managed_proton(resolved) {
                return Err(RuntimeResolutionError::RunnerRequestMismatch);
            }
            match &probe.identity.provenance {
                ComponentProvenance::ArtifactReceipt(receipt)
                    if receipt.identity.artifact_id == *artifact_id =>
                {
                    let _verification = receipt.payload_verification;
                }
                _ => return Err(RuntimeResolutionError::ProbeProvenanceMismatch),
            }
        }
        RunnerRequest::External {
            kind_hint,
            entrypoint,
        } => {
            if *kind_hint != resolved.kind()
                || !paths_match(entrypoint, resolved.runner_path())
                || !matches!(
                    probe.identity.provenance,
                    ComponentProvenance::ExternalObserved(_)
                )
            {
                return Err(RuntimeResolutionError::RunnerRequestMismatch);
            }
        }
    }
    if probe.identity.observed_material.roles.is_empty() {
        return Err(RuntimeResolutionError::ProbeProvenanceMismatch);
    }
    Ok(())
}

fn validate_capabilities(
    kind: RunnerKind,
    probe: &RunnerProbe,
) -> Result<(), RuntimeResolutionError> {
    let expected_sync = match probe.capabilities.sync_support {
        SyncSupport::RunnerManaged if kind == RunnerKind::Proton => SyncPlan::RunnerManaged,
        SyncSupport::Wine {
            fsync_patch_pair_declared: true,
            ..
        } if kind == RunnerKind::Wine => SyncPlan::Fsync,
        SyncSupport::Wine {
            esync_declared: true,
            fsync_patch_pair_declared: false,
        } if kind == RunnerKind::Wine => SyncPlan::Esync,
        SyncSupport::Wine {
            esync_declared: false,
            fsync_patch_pair_declared: false,
        } if kind == RunnerKind::Wine => SyncPlan::WineServer,
        _ => return Err(RuntimeResolutionError::CapabilityContradiction),
    };
    if expected_sync != probe.sync {
        return Err(RuntimeResolutionError::CapabilityContradiction);
    }

    if matches!(
        probe.capabilities.wow64_layout,
        CapabilityEvidence::Known {
            value: Wow64Layout::OldWow64,
            ..
        }
    ) {
        let CapabilityEvidence::Known { value, .. } =
            &probe.capabilities.supported_prefix_architectures
        else {
            return Err(RuntimeResolutionError::CapabilityContradiction);
        };
        if !value.contains(&PrefixArchitecture::X86) || !value.contains(&PrefixArchitecture::X86_64)
        {
            return Err(RuntimeResolutionError::CapabilityContradiction);
        }
    }
    Ok(())
}

fn resolve_dxvk_provider(runner: &RunnerPlan) -> Result<DxvkProvider, RuntimeResolutionError> {
    match runner.resolved.kind() {
        RunnerKind::Proton => Ok(DxvkProvider::runner_owned(
            ComponentId::new(RUNNER_DXVK_COMPONENT)
                .map_err(|_| RuntimeResolutionError::InvalidKnownId)?,
        )),
        RunnerKind::Wine if reported_wine_7_16(runner) => Ok(DxvkProvider::managed_prefix(
            ArtifactId::new(MANAGED_DXVK_COMPONENT)
                .map_err(|_| RuntimeResolutionError::InvalidKnownId)?,
        )),
        RunnerKind::Wine => Ok(DxvkProvider::winetricks_prefix(
            ComponentProvenance::LegacyUnpinned(LegacyUnpinned {
                recipe_id: WINETRICKS_DXVK_RECIPE.to_string(),
            }),
        )),
    }
}

fn reported_wine_7_16(runner: &RunnerPlan) -> bool {
    matches!(
        &runner.capabilities.reported_version,
        CapabilityEvidence::Known { value, .. } if is_wine_7_16_version(value)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::runners::managed_proton_path;
    use crate::tools::runtime::probe::probe_runner;
    use crate::utils::PrefixScope;
    use std::ops::Deref;
    use std::path::PathBuf;

    fn prefix() -> PrefixLocation {
        PrefixLocation {
            path: "/tmp/runtime-shadow-prefix".to_string(),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some("test".to_string()),
        }
    }

    fn external_profile(runner: &ResolvedRunner, graphics: GraphicsProfile) -> RuntimeProfile {
        RuntimeProfile::new(
            RunnerRequest::External {
                kind_hint: runner.kind(),
                entrypoint: runner.runner_path().to_path_buf(),
            },
            RunnerSelectionSource::GlobalSetting,
            graphics,
        )
    }

    fn test_prefix_binding(location: PrefixLocation) -> PrefixBinding {
        use crate::tools::runtime::fingerprint::{
            compute_prefix_fingerprint, managed_proton_prefix_fingerprint_input,
        };
        use crate::tools::runtime::identity::{PrefixIdentityStatus, RuntimeEligibility};
        PrefixBinding {
            status: PrefixIdentityStatus::Unknown,
            desired_fingerprint: compute_prefix_fingerprint(
                &managed_proton_prefix_fingerprint_input(),
            ),
            location,
            eligibility: RuntimeEligibility::Eligible,
        }
    }

    fn resolve(runner: &ResolvedRunner, profile: RuntimeProfile, dg: DgVoodooState) -> RuntimePlan {
        let location = prefix();
        resolve_runtime(RuntimeResolutionInput {
            profile,
            resolved: runner,
            prefix: location.clone(),
            prefix_binding: test_prefix_binding(location),
            probe: probe_runner(runner),
            dgvoodoo: dg,
            webview2_required: false,
        })
        .unwrap()
    }

    #[test]
    fn dxvk_provision_kind_matches_legacy_for_runner_policy() {
        use crate::tools::prefix::DxvkProvision;

        let proton = ResolvedRunner::test_proton(
            managed_proton_path().join("proton"),
            managed_proton_path(),
            managed_proton_path().join("umu-run"),
        );
        let probe = probe_runner(&proton);
        let runner_plan = RunnerPlan {
            resolved: proton.clone(),
            identity: probe.identity,
            capabilities: probe.capabilities,
            sync: probe.sync,
        };
        assert_eq!(
            resolve_dxvk_provider(&runner_plan)
                .expect("provider")
                .provision_kind(),
            DxvkProvision::Runner
        );
    }

    #[test]
    fn provider_table_matches_the_legacy_runner_policy() {
        let managed_proton = ResolvedRunner::test_proton(
            managed_proton_path(),
            managed_proton_path().parent().unwrap().to_path_buf(),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let managed_profile = profile_from_legacy(LegacyProfileInput {
            server_runner: None,
            default_runner: None,
            resolved: &managed_proton,
            dgvoodoo: DgVoodooState::verified(false),
        })
        .unwrap();
        assert_eq!(
            managed_profile.selection_source(),
            RunnerSelectionSource::ProductDefault
        );
        let plan = resolve(
            &managed_proton,
            managed_profile,
            DgVoodooState::verified(false),
        );
        assert_eq!(plan.graphics.dxvk_provider().kind_label(), "runner-owned");
        assert_eq!(plan.runner.sync, SyncPlan::RunnerManaged);

        let external_proton = ResolvedRunner::test_proton(
            PathBuf::from("/opt/external-proton/proton"),
            PathBuf::from("/opt/external-proton"),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let plan = resolve(
            &external_proton,
            external_profile(&external_proton, GraphicsProfile::Dxvk),
            DgVoodooState::verified(false),
        );
        assert_eq!(plan.graphics.dxvk_provider().kind_label(), "runner-owned");
        assert!(matches!(
            plan.runner.identity.provenance,
            ComponentProvenance::ExternalObserved(_)
        ));

        let wine_716 = wine_with_version("wine-7.16");
        let plan = resolve(
            &wine_716,
            external_profile(&wine_716, GraphicsProfile::Dxvk),
            DgVoodooState::verified(false),
        );
        assert_eq!(plan.graphics.dxvk_provider().kind_label(), "managed-prefix");

        let current_wine = wine_with_version("wine-10.0");
        let plan = resolve(
            &current_wine,
            external_profile(&current_wine, GraphicsProfile::Dxvk),
            DgVoodooState::verified(false),
        );
        assert_eq!(
            plan.graphics.dxvk_provider().kind_label(),
            "winetricks-prefix"
        );
    }

    #[test]
    fn profile_preserves_legacy_selection_precedence() {
        let runner = wine_with_version("wine-10.0");
        for (server_runner, default_runner, expected) in [
            (
                Some("/server/wine"),
                Some("/global/wine"),
                RunnerSelectionSource::ServerOverride,
            ),
            (
                Some(""),
                Some("/global/wine"),
                RunnerSelectionSource::GlobalSetting,
            ),
            (None, None, RunnerSelectionSource::ProductDefault),
        ] {
            let profile = profile_from_legacy(LegacyProfileInput {
                server_runner,
                default_runner,
                resolved: &runner,
                dgvoodoo: DgVoodooState::verified(false),
            })
            .unwrap();
            assert_eq!(profile.selection_source(), expected);
        }
    }

    #[test]
    fn wine_716_version_does_not_invent_old_wow64() {
        let wine = wine_with_version("wine-7.16");
        let plan = resolve(
            &wine,
            external_profile(&wine, GraphicsProfile::Dxvk),
            DgVoodooState::verified(false),
        );
        assert!(matches!(
            plan.runner.capabilities.wow64_layout,
            CapabilityEvidence::Unknown { .. }
        ));
        assert_eq!(plan.graphics.dxvk_provider().kind_label(), "managed-prefix");
    }

    #[test]
    fn dgvoodoo_requires_verified_wrappers_and_keeps_dxvk() {
        let wine = wine_with_version("wine-7.16");
        let profile = external_profile(&wine, GraphicsProfile::DgVoodooDxvk);
        let location = prefix();
        let error = resolve_runtime(RuntimeResolutionInput {
            profile,
            resolved: &wine,
            prefix: location.clone(),
            prefix_binding: test_prefix_binding(location),
            probe: probe_runner(&wine),
            dgvoodoo: DgVoodooState::unverified_configured(),
            webview2_required: false,
        })
        .unwrap_err();
        assert_eq!(error, RuntimeResolutionError::DgVoodooNotVerified);

        let profile = external_profile(&wine, GraphicsProfile::DgVoodooDxvk);
        let plan = resolve(&wine, profile, DgVoodooState::verified(true));
        assert_eq!(plan.graphics.profile(), GraphicsProfile::DgVoodooDxvk);
        assert_eq!(plan.graphics.dxvk_provider().kind_label(), "managed-prefix");
    }

    #[test]
    fn a_managed_request_cannot_wrap_an_external_proton() {
        let runner = ResolvedRunner::test_proton(
            PathBuf::from("/opt/proton/proton"),
            PathBuf::from("/opt/proton"),
            PathBuf::from("/usr/bin/umu-run"),
        );
        let profile = RuntimeProfile::new(
            RunnerRequest::Managed {
                artifact_id: ArtifactId::new(MANAGED_RUNNER_ID).unwrap(),
            },
            RunnerSelectionSource::ProductDefault,
            GraphicsProfile::Dxvk,
        );
        let location = prefix();
        let error = resolve_runtime(RuntimeResolutionInput {
            profile,
            resolved: &runner,
            prefix: location.clone(),
            prefix_binding: test_prefix_binding(location),
            probe: probe_runner(&runner),
            dgvoodoo: DgVoodooState::verified(false),
            webview2_required: false,
        })
        .unwrap_err();
        assert_eq!(error, RuntimeResolutionError::RunnerRequestMismatch);
    }

    struct TestWine {
        runner: ResolvedRunner,
        root: PathBuf,
    }

    impl Deref for TestWine {
        type Target = ResolvedRunner;

        fn deref(&self) -> &Self::Target {
            &self.runner
        }
    }

    impl Drop for TestWine {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn wine_with_version(version: &str) -> TestWine {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-resolver-wine-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        let wine = root.join("bin/wine");
        let wineserver = root.join("bin/wineserver");
        std::fs::write(&wine, format!("#!/bin/sh\necho '{version}'\n")).unwrap();
        std::fs::write(&wineserver, "#!/bin/sh\nexit 0\n").unwrap();
        for path in [&wine, &wineserver] {
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).unwrap();
        }
        TestWine {
            runner: ResolvedRunner::test_wine(wine, wineserver),
            root,
        }
    }
}
