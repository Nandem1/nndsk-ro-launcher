use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::Deserialize;

use super::descriptor::{
    ArtifactDescriptor, ExpectedDigest, PayloadValidatorId, NNDSK_RUNTIME_SOURCE_COMMIT,
};

pub(crate) const DXVK_DLLS: [&str; 5] = [
    "d3d8.dll",
    "d3d9.dll",
    "d3d10core.dll",
    "d3d11.dll",
    "dxgi.dll",
];

pub(crate) fn payload_ready(descriptor: &ArtifactDescriptor, root: &Path) -> bool {
    match descriptor.payload {
        PayloadValidatorId::UmuZipapp => umu_zipapp_ready(root),
        PayloadValidatorId::DxvkPrefixDlls => dxvk_prefix_dlls_ready(root),
        PayloadValidatorId::ProtonCachyosSlr => proton_cachyos_slr_ready(root),
        PayloadValidatorId::WineTkg716 => wine716_modules_ready(root, &WINE716_MODULES),
        PayloadValidatorId::NndskRoProton => {
            proton_cachyos_slr_ready(root)
                && ["proton", "files/bin/wine", "files/bin/wineserver"]
                    .iter()
                    .all(|entrypoint| {
                        let path = root.join(entrypoint);
                        regular_path_inside_root(root, &path) && is_executable(&path)
                    })
                && nndsk_manifest_ready(root, &NNDSK_MODULES, descriptor.version)
        }
    }
}

#[derive(Clone, Copy)]
struct ModuleIdentity {
    path: &'static str,
    sha256: &'static str,
    size: u64,
    mode: u32,
}

// Original Kron4ek 7.16 artifact: these also match the validated local TkG install.
const WINE716_MODULES: [ModuleIdentity; 6] = [
    ModuleIdentity {
        path: "bin/wine",
        size: 12_020,
        mode: 0o755,
        sha256: "0fb1461d7db1b83c15418d7d320eccf2c9c1cf09d8d97b862db96d7b58d8d331",
    },
    ModuleIdentity {
        path: "bin/wine64",
        size: 13_352,
        mode: 0o755,
        sha256: "10f12041dba2ee48d700a6b7d3fc8370c00466601b6ef88e4a901cf06a2df4ba",
    },
    ModuleIdentity {
        path: "bin/wineserver",
        size: 738_032,
        mode: 0o755,
        sha256: "d4ed4b7600247d5bee4d2790e2c0f810b3655985432e4fae5d704e64a0f358b1",
    },
    ModuleIdentity {
        path: "wine-tkg-config.txt",
        size: 4_788,
        mode: 0o744,
        sha256: "71c69694612df0f62a7b5d5252d96d383ff49d566f7806283e00a03b1e27572a",
    },
    ModuleIdentity {
        path: "lib/wine/i386-unix/ntdll.so",
        size: 668_636,
        mode: 0o755,
        sha256: "c5abd03471e51c84a4324f571c6b3d35d0eca482a07c37738b7d7dde395d4642",
    },
    ModuleIdentity {
        path: "lib/wine/x86_64-unix/ntdll.so",
        size: 737_144,
        mode: 0o755,
        sha256: "0779d6de1994a6114b575847c911d1a206ffa8b1732bdf247dfd3a3a9053af6d",
    },
];

fn wine716_modules_ready(root: &Path, modules: &[ModuleIdentity]) -> bool {
    use std::os::unix::fs::PermissionsExt;
    modules.iter().all(|module| {
        let path = root.join(module.path);
        regular_path_inside_root(root, &path)
            && path.metadata().is_ok_and(|metadata| {
                metadata.len() == module.size
                    && metadata.permissions().mode() & 0o777 == module.mode
            })
            && super::fetch::digest_file(&path, ExpectedDigest::Sha256(module.sha256))
                .is_ok_and(|hash| hash == module.sha256)
    })
}

// These are the six source-built modules in the accepted, checksum-pinned archive.
const NNDSK_MODULES: [ModuleIdentity; 6] = [
    ModuleIdentity {
        path: "files/lib/wine/i386-unix/ntdll.so",
        sha256: "2b328add78d1544dd1956bd2ab2bf27ee90d39ab69bf00ba1a0048442c1c2d22",
        size: 3_762_248,
        mode: 0o555,
    },
    ModuleIdentity {
        path: "files/lib/wine/i386-windows/crypt32.dll",
        sha256: "764670864e70c70bf0fe844c0f48772a930d0e245aa19f7e29eaeebe510b5793",
        size: 958_464,
        mode: 0o555,
    },
    ModuleIdentity {
        path: "files/lib/wine/i386-windows/ncrypt.dll",
        sha256: "954b95ad53bb9189f9805ec453306de65ec18fa318fffe83b89bb5aabfb57519",
        size: 94_208,
        mode: 0o555,
    },
    ModuleIdentity {
        path: "files/lib/wine/x86_64-unix/ntdll.so",
        sha256: "496fc61b5c942f84d5e1d29d2f1bd53571947ff1a5794074eec1a3549329ce22",
        size: 4_720_616,
        mode: 0o555,
    },
    ModuleIdentity {
        path: "files/lib/wine/x86_64-windows/crypt32.dll",
        sha256: "d5230bce23312d1761cf3fc851d73dc7babedb11ae348d0d6b4198daaecbe432",
        size: 999_424,
        mode: 0o555,
    },
    ModuleIdentity {
        path: "files/lib/wine/x86_64-windows/ncrypt.dll",
        sha256: "c0ecee51c66aa001b382520b2f1440eff748531aef0eab2622ffb3445c913d5c",
        size: 90_112,
        mode: 0o555,
    },
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NndskManifest {
    schema_version: u32,
    runtime_id: String,
    version: String,
    source_commit: String,
    patchset_revision: u64,
    requires_verified_dos_launch_path: bool,
    architectures: Vec<String>,
    entrypoints: BTreeMap<String, String>,
    modified_modules: BTreeMap<String, ManifestModule>,
    upstream: ManifestUpstream,
}

#[derive(Deserialize)]
struct ManifestModule {
    sha256: String,
    size: u64,
    mode: u32,
}

#[derive(Deserialize)]
struct ManifestUpstream {
    proton: ManifestSource,
    wine: ManifestSource,
}

#[derive(Deserialize)]
struct ManifestSource {
    commit: String,
    repository: String,
}

fn nndsk_manifest_ready(root: &Path, modules: &[ModuleIdentity], expected_version: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let manifest_path = root.join("nndsk-runtime.json");
    if !regular_path_inside_root(root, &manifest_path)
        || !manifest_path
            .metadata()
            .is_ok_and(|metadata| metadata.len() <= 65_536)
    {
        return false;
    }
    let Ok(bytes) = std::fs::read(manifest_path) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_slice::<NndskManifest>(&bytes) else {
        return false;
    };
    let architectures: BTreeSet<_> = manifest.architectures.iter().map(String::as_str).collect();
    let expected_architectures = BTreeSet::from(["PE32", "PE64", "i386-unix", "x86_64-unix"]);
    let expected_entrypoints = BTreeMap::from([
        ("proton".to_string(), "proton".to_string()),
        ("wine".to_string(), "files/bin/wine".to_string()),
        ("wineserver".to_string(), "files/bin/wineserver".to_string()),
    ]);
    if manifest.schema_version != 1
        || manifest.runtime_id != "nndsk-ro-proton"
        || manifest.version != expected_version
        || manifest.source_commit != NNDSK_RUNTIME_SOURCE_COMMIT
        || manifest.patchset_revision != 1
        || !manifest.requires_verified_dos_launch_path
        || architectures != expected_architectures
        || manifest.architectures.len() != expected_architectures.len()
        || manifest.entrypoints != expected_entrypoints
        || manifest.upstream.proton.commit != "3edf6fbb8af940de5c65b9dd0fbf366b51a218a8"
        || manifest.upstream.proton.repository != "https://github.com/CachyOS/proton-cachyos.git"
        || manifest.upstream.wine.commit != "b5f2dc7b5906ef864f83df8fef94c9f539eaad2d"
        || manifest.upstream.wine.repository != "https://github.com/CachyOS/wine-cachyos.git"
        || manifest.modified_modules.len() != modules.len()
    {
        return false;
    }
    modules.iter().all(|expected| {
        let Some(record) = manifest.modified_modules.get(expected.path) else {
            return false;
        };
        if record.sha256 != expected.sha256
            || record.size != expected.size
            || record.mode != expected.mode
        {
            return false;
        }
        let path = root.join(expected.path);
        regular_path_inside_root(root, &path)
            && path.metadata().is_ok_and(|metadata| {
                metadata.len() == expected.size
                    && metadata.permissions().mode() & 0o777 == expected.mode
            })
            && super::fetch::digest_file(&path, ExpectedDigest::Sha256(expected.sha256))
                .is_ok_and(|digest| digest == expected.sha256)
    })
}

fn regular_path_inside_root(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return false;
        };
        current.push(part);
        if current
            .symlink_metadata()
            .map_or(true, |metadata| metadata.file_type().is_symlink())
        {
            return false;
        }
    }
    is_regular_file(path)
}

fn umu_zipapp_ready(root: &Path) -> bool {
    is_executable(&root.join("umu-run")) && is_regular_file(&root.join("umu-run"))
}

fn dxvk_prefix_dlls_ready(root: &Path) -> bool {
    ["x32", "x64"].iter().all(|arch| {
        DXVK_DLLS
            .iter()
            .all(|dll| is_regular_file(&root.join(arch).join(dll)))
    })
}

fn proton_cachyos_slr_ready(root: &Path) -> bool {
    if !(is_executable(&root.join("proton")) && is_regular_file(&root.join("proton"))) {
        return false;
    }
    if !(is_executable(&root.join("files/bin/wine"))
        && is_regular_file(&root.join("files/bin/wine")))
    {
        return false;
    }

    let dxvk = root.join("files/lib/wine/dxvk");
    let dxvk_ready = ["x86_64-windows", "i386-windows"].iter().all(|arch| {
        ["d3d9.dll", "d3d11.dll", "dxgi.dll"]
            .iter()
            .all(|dll| is_regular_file(&dxvk.join(arch).join(dll)))
    });
    let vkd3d = root.join("files/lib/vkd3d");
    let vkd3d_ready = ["x86_64-windows", "i386-windows"].iter().all(|arch| {
        [
            "libvkd3d-1.dll",
            "libvkd3d-shader-1.dll",
            "libvkd3d-utils-1.dll",
        ]
        .iter()
        .all(|dll| is_regular_file(&vkd3d.join(arch).join(dll)))
    });
    dxvk_ready && vkd3d_ready
}

pub(crate) fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.is_file()
        && path
            .metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

pub(crate) fn is_regular_file(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::descriptor::{
        catalog_descriptor, LEGACY_MANAGED_RUNNER_ID, MANAGED_DXVK_ID,
    };
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn proton_readiness_requires_both_dxvk_architectures_and_d3d11() {
        let descriptor = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-artifact-payload-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        for executable in ["proton", "files/bin/wine"] {
            let path = root.join(executable);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"runner").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(!payload_ready(descriptor, &root));

        for arch in ["x86_64-windows", "i386-windows"] {
            let dxvk = root.join("files/lib/wine/dxvk").join(arch);
            let vkd3d = root.join("files/lib/vkd3d").join(arch);
            std::fs::create_dir_all(&dxvk).unwrap();
            std::fs::create_dir_all(&vkd3d).unwrap();
            for dll in ["d3d9.dll", "d3d11.dll", "dxgi.dll"] {
                std::fs::write(dxvk.join(dll), b"dxvk").unwrap();
            }
            for dll in [
                "libvkd3d-1.dll",
                "libvkd3d-shader-1.dll",
                "libvkd3d-utils-1.dll",
            ] {
                std::fs::write(vkd3d.join(dll), b"vkd3d").unwrap();
            }
        }

        assert!(payload_ready(descriptor, &root));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_dxvk_requires_every_dll_for_both_architectures() {
        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-dxvk-payload-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        for arch in ["x32", "x64"] {
            let directory = root.join(arch);
            std::fs::create_dir_all(&directory).unwrap();
            for dll in DXVK_DLLS {
                std::fs::write(directory.join(dll), b"dxvk").unwrap();
            }
        }

        assert!(payload_ready(descriptor, &root));
        std::fs::remove_file(root.join("x32/d3d9.dll")).unwrap();
        assert!(!payload_ready(descriptor, &root));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn payload_dxvk_rejects_symlink_dll() {
        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-dxvk-symlink-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(root.join("x32")).unwrap();
        std::fs::write(root.join("x32/real.dll"), b"x").unwrap();
        std::os::unix::fs::symlink("real.dll", root.join("x32/d3d9.dll")).unwrap();
        assert!(!payload_ready(descriptor, &root));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn payload_proton_accepts_internal_relative_symlink_outside_required_paths() {
        let descriptor = catalog_descriptor(LEGACY_MANAGED_RUNNER_ID).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-proton-symlink-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        for executable in ["proton", "files/bin/wine"] {
            let path = root.join(executable);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"runner").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        for arch in ["x86_64-windows", "i386-windows"] {
            let dxvk = root.join("files/lib/wine/dxvk").join(arch);
            let vkd3d = root.join("files/lib/vkd3d").join(arch);
            std::fs::create_dir_all(&dxvk).unwrap();
            std::fs::create_dir_all(&vkd3d).unwrap();
            for dll in ["d3d9.dll", "d3d11.dll", "dxgi.dll"] {
                std::fs::write(dxvk.join(dll), b"dxvk").unwrap();
            }
            for dll in [
                "libvkd3d-1.dll",
                "libvkd3d-shader-1.dll",
                "libvkd3d-utils-1.dll",
            ] {
                std::fs::write(vkd3d.join(dll), b"vkd3d").unwrap();
            }
        }
        std::fs::create_dir_all(root.join("protonfixes/extra")).unwrap();
        std::fs::write(root.join("protonfixes/extra/target.py"), b"x").unwrap();
        std::os::unix::fs::symlink("target.py", root.join("protonfixes/extra/link.py")).unwrap();
        assert!(payload_ready(descriptor, &root));
        std::fs::remove_dir_all(root).unwrap();
    }

    struct ManifestFixture {
        root: std::path::PathBuf,
        modules: Vec<ModuleIdentity>,
        manifest: serde_json::Value,
    }

    impl ManifestFixture {
        fn new() -> Self {
            use sha2::{Digest, Sha256};
            let root = std::env::temp_dir().join(format!(
                "ro-launcher-manifest-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let digest = Box::leak(format!("{:x}", Sha256::digest(b"module")).into_boxed_str());
            let modules: Vec<_> = NNDSK_MODULES
                .iter()
                .map(|module| ModuleIdentity {
                    path: module.path,
                    sha256: digest,
                    size: 6,
                    mode: 0o555,
                })
                .collect();
            let mut records = serde_json::Map::new();
            for module in &modules {
                let path = root.join(module.path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, b"module").unwrap();
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(module.mode))
                    .unwrap();
                records.insert(module.path.to_string(), serde_json::json!({"sha256": module.sha256, "size": module.size, "mode": module.mode}));
            }
            let manifest = serde_json::json!({
                "schemaVersion": 1, "runtimeId": "nndsk-ro-proton", "version": "0.1.0-dev.1",
                "sourceCommit": NNDSK_RUNTIME_SOURCE_COMMIT, "patchsetRevision": 1,
                "requiresVerifiedDosLaunchPath": true,
                "architectures": ["PE32", "PE64", "i386-unix", "x86_64-unix"],
                "entrypoints": {"proton": "proton", "wine": "files/bin/wine", "wineserver": "files/bin/wineserver"},
                "modifiedModules": records,
                "upstream": {
                    "proton": {"commit": "3edf6fbb8af940de5c65b9dd0fbf366b51a218a8", "repository": "https://github.com/CachyOS/proton-cachyos.git"},
                    "wine": {"commit": "b5f2dc7b5906ef864f83df8fef94c9f539eaad2d", "repository": "https://github.com/CachyOS/wine-cachyos.git"},
                },
            });
            let fixture = Self {
                root,
                modules,
                manifest,
            };
            fixture.save();
            fixture
        }

        fn save(&self) {
            std::fs::write(
                self.root.join("nndsk-runtime.json"),
                serde_json::to_vec(&self.manifest).unwrap(),
            )
            .unwrap();
        }
        fn ready(&self) -> bool {
            nndsk_manifest_ready(&self.root, &self.modules, "0.1.0-dev.1")
        }
    }

    impl Drop for ManifestFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn nndsk_manifest_binds_source_architectures_and_exact_modules() {
        let mut fixture = ManifestFixture::new();
        assert!(fixture.ready());
        fixture.manifest["sourceCommit"] = "foreign".into();
        fixture.save();
        assert!(!fixture.ready());
        fixture.manifest["sourceCommit"] = NNDSK_RUNTIME_SOURCE_COMMIT.into();
        fixture.manifest["architectures"] = serde_json::json!(["PE64"]);
        fixture.save();
        assert!(!fixture.ready());
        fixture.manifest["architectures"] =
            serde_json::json!(["PE32", "PE64", "i386-unix", "x86_64-unix"]);
        fixture.manifest["modifiedModules"]["extra.dll"] =
            serde_json::json!({"sha256": "00", "size": 0, "mode": 365});
        fixture.save();
        assert!(!fixture.ready());
    }

    #[test]
    fn notice_only_revision_keeps_exact_modules_but_requires_its_own_version() {
        let mut fixture = ManifestFixture::new();
        assert!(!nndsk_manifest_ready(
            &fixture.root,
            &fixture.modules,
            "0.1.0-dev.2"
        ));
        fixture.manifest["version"] = "0.1.0-dev.2".into();
        fixture.save();
        assert!(nndsk_manifest_ready(
            &fixture.root,
            &fixture.modules,
            "0.1.0-dev.2"
        ));
        assert!(!fixture.ready());
        fixture.manifest["modifiedModules"][fixture.modules[0].path]["sha256"] =
            "00".repeat(32).into();
        fixture.save();
        assert!(!nndsk_manifest_ready(
            &fixture.root,
            &fixture.modules,
            "0.1.0-dev.2"
        ));
    }

    #[test]
    fn nndsk_manifest_rehashes_modules_and_rejects_indirect_symlink_paths() {
        let fixture = ManifestFixture::new();
        let module = fixture.root.join(fixture.modules[0].path);
        std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!fixture.ready());
        std::fs::write(&module, b"tamper").unwrap();
        std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!fixture.ready());
        std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&module, b"module").unwrap();
        std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(fixture.ready());
        let parent = module.parent().unwrap();
        let renamed = parent.with_extension("preserved");
        std::fs::rename(parent, &renamed).unwrap();
        std::os::unix::fs::symlink(&renamed, parent).unwrap();
        assert!(!fixture.ready());
    }

    #[test]
    fn wine716_readiness_rejects_changed_modes_bytes_missing_architecture_and_symlinks() {
        use sha2::{Digest, Sha256};
        let fixture = ManifestFixture::new();
        let digest = Box::leak(format!("{:x}", Sha256::digest(b"wine")).into_boxed_str());
        let modules: Vec<_> = WINE716_MODULES
            .iter()
            .map(|module| ModuleIdentity {
                path: module.path,
                sha256: digest,
                size: 4,
                mode: module.mode,
            })
            .collect();
        for module in &modules {
            let path = fixture.root.join(module.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"wine").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(module.mode)).unwrap();
        }
        assert!(wine716_modules_ready(&fixture.root, &modules));
        let loader = fixture.root.join("bin/wine");
        std::fs::set_permissions(&loader, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!wine716_modules_ready(&fixture.root, &modules));
        std::fs::set_permissions(&loader, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&loader, b"fake").unwrap();
        assert!(!wine716_modules_ready(&fixture.root, &modules));
        std::fs::write(&loader, b"wine").unwrap();
        let unix32 = fixture.root.join("lib/wine/i386-unix/ntdll.so");
        std::fs::remove_file(&unix32).unwrap();
        assert!(!wine716_modules_ready(&fixture.root, &modules));
        std::os::unix::fs::symlink(&loader, &unix32).unwrap();
        assert!(!wine716_modules_ready(&fixture.root, &modules));
    }

    fn unique_suffix() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}
