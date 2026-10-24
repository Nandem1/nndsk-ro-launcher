use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use crate::utils::pe::{pe_architecture, PeArchitecture};
use serde::Deserialize;

use super::descriptor::{
    ArtifactDescriptor, ExpectedDigest, PayloadValidatorId, NNDSK_DEV3_SOURCE_COMMIT,
    NNDSK_RUNTIME_SOURCE_COMMIT,
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
        PayloadValidatorId::WineTkg716 => {
            wine716_modules_ready(root, &WINE716_MODULES, &WINE716_PE_MODULES)
        }
        PayloadValidatorId::NndskRoProton => {
            proton_cachyos_slr_ready(root)
                && ["proton", "files/bin/wine", "files/bin/wineserver"]
                    .iter()
                    .all(|entrypoint| {
                        let path = root.join(entrypoint);
                        regular_path_inside_root(root, &path) && is_executable(&path)
                    })
                && match descriptor.version {
                    "0.1.0-dev.1" | "0.1.0-dev.2" => {
                        nndsk_manifest_ready(root, &NNDSK_MODULES, &[], descriptor.version)
                    }
                    "0.1.0-dev.3" => nndsk_manifest_ready(
                        root,
                        &NNDSK_DEV3_MODULES,
                        &NNDSK_DEV3_ASSETS,
                        descriptor.version,
                    ),
                    _ => false,
                }
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

#[derive(Clone, Copy)]
struct PeModuleIdentity {
    module: ModuleIdentity,
    architecture: PeArchitecture,
}

// Inspected in the size/SHA256-verified original publisher archive, not inferred from paths.
const WINE716_PE_MODULES: [PeModuleIdentity; 2] = [
    PeModuleIdentity {
        module: ModuleIdentity {
            path: "lib/wine/i386-windows/ntdll.dll",
            size: 878_795,
            mode: 0o644,
            sha256: "3a3886bf06e0f275ec24af33d580d4734e1aea61008a0a5188168e23cf0fd463",
        },
        architecture: PeArchitecture::I386,
    },
    PeModuleIdentity {
        module: ModuleIdentity {
            path: "lib/wine/x86_64-windows/ntdll.dll",
            size: 834_838,
            mode: 0o644,
            sha256: "a97c91a3963c86f7e9cfc6bae90256ea2919aea8d8995aef816b1988fd6af43d",
        },
        architecture: PeArchitecture::Amd64,
    },
];

fn wine716_modules_ready(
    root: &Path,
    modules: &[ModuleIdentity],
    pe_modules: &[PeModuleIdentity],
) -> bool {
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
    }) && pe_modules
        .iter()
        .all(|module| pe_module_ready(root, module))
}

fn pe_module_ready(root: &Path, expected: &PeModuleIdentity) -> bool {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let module = &expected.module;
    let path = root.join(module.path);
    if !regular_path_inside_root(root, &path) {
        return false;
    }
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    else {
        return false;
    };
    if !file.metadata().is_ok_and(|metadata| {
        metadata.is_file()
            && metadata.len() == module.size
            && metadata.permissions().mode() & 0o7777 == module.mode
    }) {
        return false;
    }
    // Bounded to the pinned size, even if a file grows after the metadata check. Hash and
    // architecture inspect the same bytes; neither a FIFO nor a final symlink can be opened.
    let Some(limit) = module.size.checked_add(1) else {
        return false;
    };
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).is_ok()
        && bytes.len() as u64 == module.size
        && format!("{:x}", Sha256::digest(&bytes)) == module.sha256
        && pe_architecture(&bytes) == Some(expected.architecture)
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

// dev.3 keeps the four crypt32/ntdll modules byte-identical to dev.1/dev.2.
const NNDSK_DEV3_MODULES: [ModuleIdentity; 6] = [
    NNDSK_MODULES[0],
    NNDSK_MODULES[1],
    ModuleIdentity {
        path: "files/lib/wine/i386-windows/ncrypt.dll",
        sha256: "558510b5b41efe47c324eccb03eb759f8c43f7d2cbcc8c3d215eb6b9ba1fd829",
        size: 155_648,
        mode: 0o555,
    },
    NNDSK_MODULES[3],
    NNDSK_MODULES[4],
    ModuleIdentity {
        path: "files/lib/wine/x86_64-windows/ncrypt.dll",
        sha256: "53605884bfca9676d098bf35077461ee8f2e663b694f68374e0bdde60d57855d",
        size: 180_224,
        mode: 0o555,
    },
];

const NNDSK_DEV3_ASSETS: [ModuleIdentity; 3] = [
    ModuleIdentity {
        path: "proton",
        sha256: "8a2f685fdfdc8753cc1a9503e141f893cb907c18a561f2d5ee33c3aa1c9abd71",
        size: 112_641,
        mode: 0o755,
    },
    ModuleIdentity {
        path: "nndsk_volume_aliases.py",
        sha256: "5174a32911482fa7ac5335bebfed83e1c94e80dffca9db46ce3b0d26d85635f3",
        size: 2_669,
        mode: 0o644,
    },
    ModuleIdentity {
        path: "files/share/nndsk/c-volume-query.exe",
        sha256: "8b6132e7a7af5bac3d71a1712b007bc605c9b52bc4361a29a1811197b66b908e",
        size: 2_560,
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
    #[serde(default)]
    compatibility_assets: BTreeMap<String, ManifestModule>,
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

fn nndsk_manifest_ready(
    root: &Path,
    modules: &[ModuleIdentity],
    assets: &[ModuleIdentity],
    expected_version: &str,
) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let (source_commit, revision) = match expected_version {
        "0.1.0-dev.1" | "0.1.0-dev.2" => (NNDSK_RUNTIME_SOURCE_COMMIT, 1),
        "0.1.0-dev.3" => (NNDSK_DEV3_SOURCE_COMMIT, 2),
        _ => return false,
    };

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
        || manifest.source_commit != source_commit
        || manifest.patchset_revision != revision
        || !manifest.requires_verified_dos_launch_path
        || architectures != expected_architectures
        || manifest.architectures.len() != expected_architectures.len()
        || manifest.entrypoints != expected_entrypoints
        || manifest.upstream.proton.commit != "3edf6fbb8af940de5c65b9dd0fbf366b51a218a8"
        || manifest.upstream.proton.repository != "https://github.com/CachyOS/proton-cachyos.git"
        || manifest.upstream.wine.commit != "b5f2dc7b5906ef864f83df8fef94c9f539eaad2d"
        || manifest.upstream.wine.repository != "https://github.com/CachyOS/wine-cachyos.git"
        || manifest.modified_modules.len() != modules.len()
        || manifest.compatibility_assets.len() != assets.len()
    {
        return false;
    }
    modules
        .iter()
        .map(|module| (&manifest.modified_modules, module))
        .chain(
            assets
                .iter()
                .map(|asset| (&manifest.compatibility_assets, asset)),
        )
        .all(|(records, expected)| {
            let Some(record) = records.get(expected.path) else {
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
                        && metadata.permissions().mode() & 0o7777 == expected.mode
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
    is_executable(&root.join("umu-run")) && regular_path_inside_root(root, &root.join("umu-run"))
}

fn dxvk_prefix_dlls_ready(root: &Path) -> bool {
    ["x32", "x64"].iter().all(|arch| {
        DXVK_DLLS
            .iter()
            .all(|dll| regular_path_inside_root(root, &root.join(arch).join(dll)))
    })
}

fn proton_cachyos_slr_ready(root: &Path) -> bool {
    if !(is_executable(&root.join("proton"))
        && regular_path_inside_root(root, &root.join("proton")))
    {
        return false;
    }
    if !(is_executable(&root.join("files/bin/wine"))
        && regular_path_inside_root(root, &root.join("files/bin/wine")))
    {
        return false;
    }

    let dxvk = root.join("files/lib/wine/dxvk");
    let dxvk_ready = ["x86_64-windows", "i386-windows"].iter().all(|arch| {
        ["d3d9.dll", "d3d11.dll", "dxgi.dll"]
            .iter()
            .all(|dll| regular_path_inside_root(root, &dxvk.join(arch).join(dll)))
    });
    let vkd3d = root.join("files/lib/vkd3d");
    let vkd3d_ready = ["x86_64-windows", "i386-windows"].iter().all(|arch| {
        [
            "libvkd3d-1.dll",
            "libvkd3d-shader-1.dll",
            "libvkd3d-utils-1.dll",
        ]
        .iter()
        .all(|dll| regular_path_inside_root(root, &vkd3d.join(arch).join(dll)))
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
            nndsk_manifest_ready(&self.root, &self.modules, &[], "0.1.0-dev.1")
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
            &[],
            "0.1.0-dev.2"
        ));
        fixture.manifest["version"] = "0.1.0-dev.2".into();
        fixture.save();
        assert!(nndsk_manifest_ready(
            &fixture.root,
            &fixture.modules,
            &[],
            "0.1.0-dev.2"
        ));
        assert!(!fixture.ready());
        fixture.manifest["modifiedModules"][fixture.modules[0].path]["sha256"] =
            "00".repeat(32).into();
        fixture.save();
        assert!(!nndsk_manifest_ready(
            &fixture.root,
            &fixture.modules,
            &[],
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
    fn installer_revision_keeps_crypt32_ntdll_and_replaces_only_ncrypt() {
        for index in [0, 1, 3, 4] {
            assert_eq!(
                NNDSK_MODULES[index].sha256,
                NNDSK_DEV3_MODULES[index].sha256
            );
            assert_eq!(NNDSK_MODULES[index].size, NNDSK_DEV3_MODULES[index].size);
        }
        for index in [2, 5] {
            assert_eq!(NNDSK_MODULES[index].path, NNDSK_DEV3_MODULES[index].path);
            assert_ne!(
                NNDSK_MODULES[index].sha256,
                NNDSK_DEV3_MODULES[index].sha256
            );
        }
    }

    #[test]
    fn installer_revision_binds_new_source_revision_and_exact_compatibility_assets() {
        use sha2::{Digest, Sha256};
        let mut fixture = ManifestFixture::new();
        fixture.manifest["version"] = "0.1.0-dev.3".into();
        let digest = Box::leak(format!("{:x}", Sha256::digest(b"asset")).into_boxed_str());
        let assets: Vec<_> = NNDSK_DEV3_ASSETS
            .iter()
            .map(|asset| ModuleIdentity {
                path: asset.path,
                sha256: digest,
                size: 5,
                mode: asset.mode,
            })
            .collect();
        let mut records = serde_json::Map::new();
        for asset in &assets {
            let path = fixture.root.join(asset.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"asset").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(asset.mode)).unwrap();
            records.insert(
                asset.path.into(),
                serde_json::json!({"sha256": asset.sha256, "size": asset.size, "mode": asset.mode}),
            );
        }
        fixture.manifest["compatibilityAssets"] = records.clone().into();
        fixture.save();
        let ready = |fixture: &ManifestFixture| {
            nndsk_manifest_ready(&fixture.root, &fixture.modules, &assets, "0.1.0-dev.3")
        };
        // Relabelling an old source/revision is insufficient, even with the helper files.
        assert!(!ready(&fixture));
        fixture.manifest["sourceCommit"] = NNDSK_DEV3_SOURCE_COMMIT.into();
        fixture.save();
        assert!(!ready(&fixture));
        fixture.manifest["patchsetRevision"] = 2.into();
        fixture.save();
        assert!(ready(&fixture));
        assert!(!nndsk_manifest_ready(
            &fixture.root,
            &fixture.modules,
            &assets,
            "0.1.0-dev.99"
        ));

        for asset in &assets {
            fixture.manifest["compatibilityAssets"]
                .as_object_mut()
                .unwrap()
                .remove(asset.path);
            fixture.save();
            assert!(!ready(&fixture));
            fixture.manifest["compatibilityAssets"] = records.clone().into();
            fixture.save();
            let path = fixture.root.join(asset.path);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(asset.mode | 0o4000))
                .unwrap();
            assert!(!ready(&fixture));
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::write(&path, b"other").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(asset.mode)).unwrap();
            assert!(!ready(&fixture));
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::write(&path, b"asset").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(asset.mode)).unwrap();
            let saved = path.with_extension("preserved");
            std::fs::rename(&path, &saved).unwrap();
            std::os::unix::fs::symlink(&saved, &path).unwrap();
            assert!(!ready(&fixture));
            std::fs::remove_file(&path).unwrap();
            std::fs::rename(&saved, &path).unwrap();
            assert!(ready(&fixture));
        }
        fixture.manifest["compatibilityAssets"]["unexpected.py"] =
            serde_json::json!({"sha256": digest, "size": 5, "mode": 420});
        fixture.save();
        assert!(!ready(&fixture));
    }

    fn wine716_fixture() -> (ManifestFixture, Vec<ModuleIdentity>, Vec<PeModuleIdentity>) {
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
        let pe_modules = WINE716_PE_MODULES
            .iter()
            .map(|expected| {
                let (machine, magic) = match expected.architecture {
                    PeArchitecture::I386 => (0x014c, 0x010b),
                    PeArchitecture::Amd64 => (0x8664, 0x020b),
                };
                write_pe_fixture(
                    &fixture.root,
                    *expected,
                    &crate::utils::pe::test_pe(machine, magic),
                )
            })
            .collect();
        (fixture, modules, pe_modules)
    }

    fn write_pe_fixture(
        root: &Path,
        mut expected: PeModuleIdentity,
        bytes: &[u8],
    ) -> PeModuleIdentity {
        use sha2::{Digest, Sha256};
        let path = root.join(expected.module.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(expected.module.mode))
            .unwrap();
        expected.module.size = bytes.len() as u64;
        expected.module.sha256 = Box::leak(format!("{:x}", Sha256::digest(bytes)).into_boxed_str());
        expected
    }

    #[test]
    fn wine716_readiness_rejects_changed_modes_bytes_missing_architecture_and_symlinks() {
        let (fixture, modules, pe_modules) = wine716_fixture();
        let ready = || wine716_modules_ready(&fixture.root, &modules, &pe_modules);
        assert!(ready());
        let loader = fixture.root.join("bin/wine");
        std::fs::set_permissions(&loader, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!ready());
        std::fs::set_permissions(&loader, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&loader, b"fake").unwrap();
        assert!(!ready());
        std::fs::write(&loader, b"wine").unwrap();
        let unix32 = fixture.root.join("lib/wine/i386-unix/ntdll.so");
        std::fs::remove_file(&unix32).unwrap();
        assert!(!ready());
        std::os::unix::fs::symlink(&loader, &unix32).unwrap();
        assert!(!ready());
    }

    #[test]
    fn wine716_pe_gate_rejects_hash_consistent_wrong_machine_class_and_malformed_headers() {
        use crate::utils::pe::test_pe;
        for slot in 0..2 {
            let (fixture, modules, pe_modules) = wine716_fixture();
            assert!(wine716_modules_ready(&fixture.root, &modules, &pe_modules));
            let valid = std::fs::read(fixture.root.join(pe_modules[slot].module.path)).unwrap();
            let mut cases = vec![
                test_pe(0xffff, 0x010b),
                test_pe(0xffff, 0x020b),
                test_pe(0x014c, 0x020b),
                test_pe(0x8664, 0x010b),
                if slot == 0 {
                    test_pe(0x8664, 0x020b)
                } else {
                    test_pe(0x014c, 0x010b)
                },
            ];
            for end in [0, 2, 0x3f, 0x84, 0x97, 0x99, valid.len() - 1] {
                cases.push(valid[..end].to_vec());
            }
            for (offset, replacement) in [
                (0, vec![0]),
                (0x80, vec![0]),
                (0x3c, u32::MAX.to_le_bytes().to_vec()),
            ] {
                let mut bytes = valid.clone();
                bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
                cases.push(bytes);
            }
            for bytes in cases {
                let mut expected = pe_modules.clone();
                // Keep path, size, mode AND hash consistent: only PE semantics can reject this.
                expected[slot] = write_pe_fixture(&fixture.root, expected[slot], &bytes);
                assert!(!wine716_modules_ready(&fixture.root, &modules, &expected));
            }
        }
    }

    #[test]
    fn wine716_pe_gate_rejects_swapped_anchors_even_with_matching_hashes() {
        let (fixture, modules, mut pe_modules) = wine716_fixture();
        assert!(wine716_modules_ready(&fixture.root, &modules, &pe_modules));
        let pe32 = std::fs::read(fixture.root.join(pe_modules[0].module.path)).unwrap();
        let pe64 = std::fs::read(fixture.root.join(pe_modules[1].module.path)).unwrap();
        pe_modules[0] = write_pe_fixture(&fixture.root, pe_modules[0], &pe64);
        pe_modules[1] = write_pe_fixture(&fixture.root, pe_modules[1], &pe32);
        assert!(!wine716_modules_ready(&fixture.root, &modules, &pe_modules));
    }

    #[test]
    fn wine716_pe_gate_rejects_missing_special_symlinked_and_corrupt_anchors() {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        for slot in 0..2 {
            let (fixture, modules, pe_modules) = wine716_fixture();
            let ready = || wine716_modules_ready(&fixture.root, &modules, &pe_modules);
            assert!(ready());
            let path = fixture.root.join(pe_modules[slot].module.path);
            let original = std::fs::read(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            assert!(!ready());
            std::fs::create_dir(&path).unwrap();
            assert!(!ready());
            std::fs::remove_dir(&path).unwrap();
            let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
            // SAFETY: a valid NUL-terminated path within this exclusively owned fixture.
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o644) }, 0);
            assert!(!ready());
            std::fs::remove_file(&path).unwrap();
            let external = fixture.root.with_extension(format!("outside-{slot}"));
            std::fs::write(&external, &original).unwrap();
            symlink(&external, &path).unwrap();
            assert!(!ready());
            std::fs::remove_file(&path).unwrap();
            std::fs::remove_file(external).unwrap();
            let mut corrupt = original.clone();
            *corrupt.last_mut().unwrap() ^= 1; // Valid architecture, same size/mode, wrong hash.
            std::fs::write(&path, corrupt).unwrap();
            assert!(!ready());
            std::fs::write(&path, &original).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(ready());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o4644)).unwrap();
            assert!(!ready());
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            let directory = path.parent().unwrap();
            let saved = directory.with_extension("saved");
            std::fs::rename(directory, &saved).unwrap();
            symlink(&saved, directory).unwrap();
            assert!(!ready());
            std::fs::remove_file(directory).unwrap();
            std::fs::rename(saved, directory).unwrap();
            assert!(ready());
        }
    }

    fn unique_suffix() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}
