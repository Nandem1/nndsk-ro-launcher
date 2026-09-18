use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::tools::runners::managed_proton_path;
use crate::utils::{ResolvedRunner, RunnerKind};

use super::managed_identity::{
    managed_proton_artifact_identity, managed_runtime_payload_verification,
};
use super::material::FileDigestCache;
use super::model::{
    ArtifactReceipt, CapabilityEvidence, CapabilitySource, ComponentProvenance,
    ExternalObserved, ObservedMaterial, ObservedMaterialRole, ObservedRunnerMaterial,
    PayloadVerification, PrefixArchitecture, RunnerCapabilities, RunnerIdentity, SyncPlan,
    SyncSupport, UnknownCapabilityReason, Wow64Layout,
};

#[derive(Debug, Clone)]
pub(crate) struct RunnerProbe {
    pub(super) identity: RunnerIdentity,
    pub(super) capabilities: RunnerCapabilities,
    pub(super) sync: SyncPlan,
}

pub(crate) fn probe_managed_proton_descriptor(payload: PayloadVerification) -> RunnerProbe {
    RunnerProbe {
        identity: RunnerIdentity {
            kind: RunnerKind::Proton,
            provenance: ComponentProvenance::ArtifactReceipt(ArtifactReceipt {
                identity: managed_proton_artifact_identity(),
                payload_verification: payload,
            }),
            observed_material: ObservedRunnerMaterial {
                roles: BTreeSet::new(),
            },
        },
        capabilities: RunnerCapabilities {
            reported_version: CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::DescriptorDoesNotDeclare,
            },
            wow64_layout: CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::DescriptorDoesNotDeclare,
            },
            supported_prefix_architectures: CapabilityEvidence::Unknown {
                reason: UnknownCapabilityReason::DescriptorDoesNotDeclare,
            },
            sync_support: SyncSupport::RunnerManaged,
        },
        sync: SyncPlan::RunnerManaged,
    }
}

pub(crate) fn probe_runner(resolved: &ResolvedRunner) -> RunnerProbe {
    let mut cache = FileDigestCache::default();
    probe_runner_with_cache(resolved, &mut cache)
}

fn probe_runner_with_cache(resolved: &ResolvedRunner, cache: &mut FileDigestCache) -> RunnerProbe {
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

    let roles = observed_roles(resolved, cache);
    let provenance = if is_managed_proton(resolved) {
        ComponentProvenance::ArtifactReceipt(ArtifactReceipt {
            identity: managed_proton_artifact_identity(),
            payload_verification: managed_runtime_payload_verification(),
        })
    } else {
        ComponentProvenance::ExternalObserved(ExternalObserved {
            roles: roles.iter().map(|material| material.role).collect(),
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

pub(crate) fn paths_match(left: &Path, right: &Path) -> bool {
    canonical_or_original(left) == canonical_or_original(right)
}

fn canonical_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn observed_roles(
    resolved: &ResolvedRunner,
    cache: &mut FileDigestCache,
) -> BTreeSet<ObservedMaterial> {
    let mut roles = BTreeSet::new();
    roles.insert(material_for_path(
        resolved.runner_path(),
        ObservedMaterialRole::Entrypoint,
        cache,
    ));
    match resolved.kind() {
        RunnerKind::Wine => {
            if let Some(path) = resolved.wineserver_path() {
                roles.insert(material_for_path(
                    path,
                    ObservedMaterialRole::WineServer,
                    cache,
                ));
            }
            if let Some(root) = wine_root(resolved) {
                let config = root.join("wine-tkg-config.txt");
                if config.is_file() {
                    roles.insert(material_for_path(
                        &config,
                        ObservedMaterialRole::WineTkgConfig,
                        cache,
                    ));
                }
            }
        }
        RunnerKind::Proton => {
            if let Some(root) = resolved.proton_root() {
                let inner = root.join("files/bin/wine");
                if inner.is_file() {
                    roles.insert(material_for_path(
                        &inner,
                        ObservedMaterialRole::ProtonInnerWine,
                        cache,
                    ));
                }
                let version = root.join("version");
                if version.is_file() {
                    roles.insert(material_for_path(
                        &version,
                        ObservedMaterialRole::ProtonVersion,
                        cache,
                    ));
                }
            }
            if let Some(path) = resolved.proton_umu_path() {
                roles.insert(material_for_path(
                    path,
                    ObservedMaterialRole::UmuEntrypoint,
                    cache,
                ));
            }
        }
    }
    roles
}

fn material_for_path(
    path: &Path,
    role: ObservedMaterialRole,
    cache: &mut FileDigestCache,
) -> ObservedMaterial {
    ObservedMaterial {
        role,
        digest: cache.sha256_file(path),
    }
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
