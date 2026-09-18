use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::tools::runners::{managed_proton_path, managed_runtime_ready, MANAGED_RUNNER_ID};
use crate::utils::{ResolvedRunner, RunnerKind};

use super::model::{
    ArtifactId, ArtifactReceipt, CapabilityEvidence, CapabilitySource, ComponentProvenance,
    ExternalObserved, ObservedMaterialRole, ObservedRunnerMaterial, PayloadVerification,
    PrefixArchitecture, RunnerCapabilities, RunnerIdentity, SyncPlan, SyncSupport,
    UnknownCapabilityReason, Wow64Layout,
};

#[derive(Debug, Clone)]
pub(super) struct RunnerProbe {
    pub(super) identity: RunnerIdentity,
    pub(super) capabilities: RunnerCapabilities,
    pub(super) sync: SyncPlan,
}

pub(super) fn probe_runner(resolved: &ResolvedRunner) -> RunnerProbe {
    let kind = resolved.kind();
    let reported_version = match resolved.reported_version() {
        Some(value) => CapabilityEvidence::Known {
            value,
            source: if kind == RunnerKind::Wine {
                CapabilitySource::ProcessOutput
            } else {
                CapabilitySource::VersionFile
            },
        },
        None => CapabilityEvidence::Unknown {
            reason: UnknownCapabilityReason::NotReported,
        },
    };

    let (wow64_layout, supported_prefix_architectures) = probe_layout(resolved);
    let (sync_support, sync) = match kind {
        RunnerKind::Proton => (SyncSupport::RunnerManaged, SyncPlan::RunnerManaged),
        RunnerKind::Wine => {
            let (esync_declared, fsync_patch_pair_declared) = wine_sync_declarations(resolved);
            match (esync_declared, fsync_patch_pair_declared) {
                (_, true) => (
                    SyncSupport::Wine {
                        esync_declared,
                        fsync_patch_pair_declared,
                    },
                    SyncPlan::Fsync,
                ),
                (true, false) => (
                    SyncSupport::Wine {
                        esync_declared,
                        fsync_patch_pair_declared,
                    },
                    SyncPlan::Esync,
                ),
                (false, false) => (
                    SyncSupport::Wine {
                        esync_declared,
                        fsync_patch_pair_declared,
                    },
                    SyncPlan::WineServer,
                ),
            }
        }
    };

    let roles = observed_roles(resolved);
    let provenance = if is_managed_proton(resolved) {
        ComponentProvenance::ArtifactReceipt(ArtifactReceipt {
            artifact_id: ArtifactId::new(MANAGED_RUNNER_ID)
                .expect("the managed runner id is a stable internal constant"),
            payload_verification: if managed_runtime_ready() {
                PayloadVerification::ShapeVerified
            } else {
                PayloadVerification::Unverified
            },
        })
    } else {
        ComponentProvenance::ExternalObserved(ExternalObserved {
            roles: roles.clone(),
            complete: false,
        })
    };

    RunnerProbe {
        identity: RunnerIdentity {
            kind,
            provenance,
            observed_material: ObservedRunnerMaterial { roles },
        },
        capabilities: RunnerCapabilities {
            reported_version,
            wow64_layout,
            supported_prefix_architectures,
            sync_support,
        },
        sync,
    }
}

pub(super) fn is_managed_proton(resolved: &ResolvedRunner) -> bool {
    resolved.kind() == RunnerKind::Proton
        && paths_match(resolved.runner_path(), &managed_proton_path())
}

pub(super) fn paths_match(left: &Path, right: &Path) -> bool {
    canonical_or_original(left) == canonical_or_original(right)
}

fn canonical_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn observed_roles(resolved: &ResolvedRunner) -> BTreeSet<ObservedMaterialRole> {
    let mut roles = BTreeSet::from([ObservedMaterialRole::Entrypoint]);
    match resolved.kind() {
        RunnerKind::Wine => {
            if resolved.wineserver_path().is_some() {
                roles.insert(ObservedMaterialRole::WineServer);
            }
            if wine_root(resolved).is_some_and(|root| root.join("wine-tkg-config.txt").is_file()) {
                roles.insert(ObservedMaterialRole::WineTkgConfig);
            }
        }
        RunnerKind::Proton => {
            if resolved
                .proton_root()
                .is_some_and(|root| root.join("files/bin/wine").is_file())
            {
                roles.insert(ObservedMaterialRole::ProtonInnerWine);
            }
            if resolved
                .proton_root()
                .is_some_and(|root| root.join("version").is_file())
            {
                roles.insert(ObservedMaterialRole::ProtonVersion);
            }
            if resolved.proton_umu_path().is_some() {
                roles.insert(ObservedMaterialRole::UmuEntrypoint);
            }
        }
    }
    roles
}

fn probe_layout(
    resolved: &ResolvedRunner,
) -> (
    CapabilityEvidence<Wow64Layout>,
    CapabilityEvidence<BTreeSet<PrefixArchitecture>>,
) {
    if resolved.kind() != RunnerKind::Wine {
        return (
            CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::DescriptorDoesNotDeclare,
            },
            CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::DescriptorDoesNotDeclare,
            },
        );
    }

    let old_wow64 = wine_root(resolved).is_some_and(|root| {
        contains_regular_file(&root.join("lib/wine/i386-unix"))
            && contains_regular_file(&root.join("lib/wine/x86_64-unix"))
    });
    if old_wow64 {
        (
            CapabilityEvidence::Known {
                value: Wow64Layout::OldWow64,
                source: CapabilitySource::StructuralProbeV1,
            },
            CapabilityEvidence::Known {
                value: BTreeSet::from([PrefixArchitecture::X86, PrefixArchitecture::X86_64]),
                source: CapabilitySource::StructuralProbeV1,
            },
        )
    } else {
        (
            CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::LayoutNotProven,
            },
            CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::LayoutNotProven,
            },
        )
    }
}

fn wine_root(resolved: &ResolvedRunner) -> Option<PathBuf> {
    canonical_or_original(resolved.runner_path())
        .parent()?
        .parent()
        .map(Path::to_path_buf)
}

fn wine_sync_declarations(resolved: &ResolvedRunner) -> (bool, bool) {
    let config = wine_root(resolved)
        .and_then(|root| std::fs::read_to_string(root.join("wine-tkg-config.txt")).ok());
    let Some(config) = config else {
        return (false, false);
    };
    (
        config.contains("Using wine-staging patchset"),
        config.contains("fsync-unix-staging.patch") && config.contains("fsync_futex_waitv.patch"),
    )
}

fn contains_regular_file(path: &Path) -> bool {
    path.read_dir().is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ro-launcher-runtime-probe-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn structural_probe_requires_both_nonempty_unix_layouts() {
        let root = test_root("wow64");
        let wine = root.join("bin/wine");
        let wineserver = root.join("bin/wineserver");
        std::fs::create_dir_all(wine.parent().unwrap()).unwrap();
        for arch in ["i386-unix", "x86_64-unix"] {
            let dir = root.join("lib/wine").join(arch);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("ntdll.so"), arch).unwrap();
        }
        let runner = ResolvedRunner::test_wine(wine, wineserver);
        let probe = probe_runner(&runner);
        assert!(matches!(
            probe.capabilities.wow64_layout,
            CapabilityEvidence::Known {
                value: Wow64Layout::OldWow64,
                ..
            }
        ));

        std::fs::remove_file(root.join("lib/wine/i386-unix/ntdll.so")).unwrap();
        let probe = probe_runner(&runner);
        assert!(matches!(
            probe.capabilities.wow64_layout,
            CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::LayoutNotProven
            }
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wine_sync_never_invents_ntsync() {
        let root = test_root("sync");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(
            root.join("wine-tkg-config.txt"),
            "fsync-unix-staging.patch\nfsync_futex_waitv.patch\n",
        )
        .unwrap();
        let runner = ResolvedRunner::test_wine(root.join("bin/wine"), root.join("bin/wineserver"));
        let probe = probe_runner(&runner);
        assert_eq!(probe.sync, SyncPlan::Fsync);
        assert!(matches!(
            probe.capabilities.sync_support,
            SyncSupport::Wine {
                fsync_patch_pair_declared: true,
                ..
            }
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
