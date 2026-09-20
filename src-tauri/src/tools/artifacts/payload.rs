use std::path::Path;

use super::descriptor::{ArtifactDescriptor, PayloadValidatorId};

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
    }
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
        catalog_descriptor, MANAGED_DXVK_ID, MANAGED_RUNNER_ID,
    };
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn proton_readiness_requires_both_dxvk_architectures_and_d3d11() {
        let descriptor = catalog_descriptor(MANAGED_RUNNER_ID).unwrap();
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
        let descriptor = catalog_descriptor(MANAGED_RUNNER_ID).unwrap();
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

    fn unique_suffix() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }
}
