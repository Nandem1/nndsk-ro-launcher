use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::utils::replace_json;

use super::descriptor::ArtifactDescriptor;
use super::receipt::{receipt_v2_from_descriptor, MARKER_FILE};

#[cfg(test)]
thread_local! {
    pub(super) static INSTALL_FAULT: std::cell::Cell<InstallFault> =
        const { std::cell::Cell::new(InstallFault::None) };
    pub(super) static BREAK_RESTORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstallFault {
    None,
    AfterActivateRename,
    AfterMarkerWrite,
    #[allow(dead_code)]
    RestoreRename,
}

pub(crate) fn install_extracted(
    descriptor: &ArtifactDescriptor,
    staging_dir: &Path,
    final_dir: &Path,
    artifact_ready: fn(&ArtifactDescriptor) -> bool,
) -> Result<(), String> {
    let extracted = staging_dir.join(descriptor.archive.archive_root());
    let backup =
        final_dir.with_file_name(format!(".{}.previous-{}", descriptor.id, unique_suffix()));
    let had_previous = final_dir.exists();
    if had_previous {
        std::fs::rename(final_dir, &backup)
            .map_err(|error| format!("No se pudo preservar el runtime anterior: {error}"))?;
    }

    let result = (|| {
        std::fs::rename(&extracted, final_dir)
            .map_err(|error| format!("No se pudo activar {}: {error}", descriptor.id))?;
        inject_fault(InstallFault::AfterActivateRename)?;

        replace_json(
            &final_dir.join(MARKER_FILE),
            &receipt_v2_from_descriptor(descriptor),
        )?;
        inject_fault(InstallFault::AfterMarkerWrite)?;

        if !artifact_ready(descriptor) {
            return Err(format!(
                "La instalación de {} no superó la validación",
                descriptor.id
            ));
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            if had_previous {
                let _ = std::fs::remove_dir_all(backup);
            }
            clear_fault();
            Ok(())
        }
        Err(error) => {
            if final_dir.exists() {
                let _ = std::fs::remove_dir_all(final_dir);
            }
            if had_previous {
                if cfg!(test) && break_restore() {
                    let _ = std::fs::remove_dir_all(&backup);
                    let _ = std::fs::write(&backup, b"not-a-directory");
                    let _ = std::fs::create_dir_all(final_dir);
                }
                match std::fs::rename(&backup, final_dir) {
                    Ok(()) => {
                        clear_fault();
                        Err(error)
                    }
                    Err(restore) => {
                        clear_fault();
                        Err(format!(
                            "{error}; además no se pudo restaurar el runtime anterior: {restore}"
                        ))
                    }
                }
            } else {
                clear_fault();
                Err(error)
            }
        }
    }
}

fn inject_fault(fault: InstallFault) -> Result<(), String> {
    if cfg!(test) && current_fault() == fault {
        return Err(format!("fault inyectado: {:?}", fault));
    }
    Ok(())
}

#[cfg(test)]
fn current_fault() -> InstallFault {
    INSTALL_FAULT.get()
}

#[cfg(not(test))]
fn current_fault() -> InstallFault {
    InstallFault::None
}

#[cfg(test)]
fn clear_fault() {
    INSTALL_FAULT.set(InstallFault::None);
    BREAK_RESTORE.set(false);
}

#[cfg(not(test))]
fn clear_fault() {}

#[cfg(test)]
fn break_restore() -> bool {
    BREAK_RESTORE.get()
}

#[cfg(not(test))]
fn break_restore() -> bool {
    false
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

pub(crate) struct StagingCleanup {
    download_path: PathBuf,
    staging_dir: PathBuf,
    armed: bool,
}

impl StagingCleanup {
    pub(crate) fn new(download_path: PathBuf, staging_dir: PathBuf) -> Self {
        Self {
            download_path,
            staging_dir,
            armed: true,
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.download_path);
            let _ = std::fs::remove_dir_all(&self.staging_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::artifact_ready;
    use crate::tools::artifacts::descriptor::{
        catalog_descriptor, MANAGED_DXVK_ID, MANAGED_RUNNER_ID,
    };
    use crate::tools::artifacts::payload::DXVK_DLLS;
    use crate::tools::artifacts::receipt::{RuntimeMarkerV1, MARKER_FILE, RUNTIME_SCHEMA_V1};

    fn unique_suffix() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }

    fn write_dxvk_payload(root: &Path) {
        for arch in ["x32", "x64"] {
            for dll in DXVK_DLLS {
                let path = root.join(arch).join(dll);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"dxvk").unwrap();
            }
        }
    }

    #[test]
    fn install_restore_on_activate_failure_keeps_previous_payload() {
        let base = std::env::temp_dir().join(format!(
            "ro-launcher-install-restore-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let runtime = base.join("runtime");
        let final_dir = runtime.join(MANAGED_DXVK_ID);
        std::fs::create_dir_all(&final_dir).unwrap();
        std::fs::write(final_dir.join("sentinel.txt"), b"keep").unwrap();
        write_dxvk_payload(&final_dir);

        let staging = runtime.join(".staging");
        let extracted = staging.join("dxvk-2.6.2");
        std::fs::create_dir_all(&extracted).unwrap();
        write_dxvk_payload(&extracted);
        std::fs::write(extracted.join("sentinel.txt"), b"new").unwrap();

        INSTALL_FAULT.set(InstallFault::AfterActivateRename);
        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        let err = install_extracted(descriptor, &staging, &final_dir, artifact_ready).unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(
            std::fs::read_to_string(final_dir.join("sentinel.txt")).unwrap(),
            "keep"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn install_restore_error_is_not_swallowed() {
        let base = std::env::temp_dir().join(format!(
            "ro-launcher-install-restore-fail-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let runtime = base.join("runtime");
        let final_dir = runtime.join(MANAGED_DXVK_ID);
        std::fs::create_dir_all(&final_dir).unwrap();
        write_dxvk_payload(&final_dir);

        let staging = runtime.join(".staging");
        let extracted = staging.join("dxvk-2.6.2");
        std::fs::create_dir_all(&extracted).unwrap();
        write_dxvk_payload(&extracted);

        INSTALL_FAULT.set(InstallFault::AfterActivateRename);
        BREAK_RESTORE.set(true);
        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        let err = install_extracted(descriptor, &staging, &final_dir, artifact_ready).unwrap_err();
        assert!(err.contains("no se pudo restaurar el runtime anterior"));
        assert!(
            base.join("runtime")
                .join(format!(".{}.previous-", MANAGED_DXVK_ID))
                .exists()
                || std::fs::read_dir(runtime).unwrap().any(|entry| entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("previous"))
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn non_zero_status_not_hidden_by_existing_files() {
        let base = std::env::temp_dir().join(format!(
            "ro-launcher-install-incomplete-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let runtime = base.join("runtime");
        let final_dir = runtime.join(MANAGED_DXVK_ID);
        std::fs::create_dir_all(&final_dir).unwrap();
        std::fs::write(final_dir.join("orphan.txt"), b"x").unwrap();

        let staging = runtime.join(".staging");
        let extracted = staging.join("dxvk-2.6.2");
        std::fs::create_dir_all(&extracted).unwrap();
        write_dxvk_payload(&extracted);

        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        assert!(install_extracted(descriptor, &staging, &final_dir, |_| false).is_err());
        assert!(final_dir.join("orphan.txt").exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn staging_cleanup_drop_removes_temps() {
        let base = std::env::temp_dir().join(format!(
            "ro-launcher-staging-cleanup-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(&base).unwrap();
        let download = base.join("download");
        let staging = base.join("staging");
        std::fs::write(&download, b"x").unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        {
            let _cleanup = StagingCleanup::new(download.clone(), staging.clone());
        }
        assert!(!download.exists());
        assert!(!staging.exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn payload_valid_with_foreign_marker_is_not_ready() {
        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-foreign-marker-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(&root).unwrap();
        write_dxvk_payload(&root);
        let foreign = RuntimeMarkerV1 {
            schema_version: RUNTIME_SCHEMA_V1,
            artifact_id: MANAGED_RUNNER_ID.to_string(),
            digest: "00".repeat(64),
        };
        crate::utils::replace_json(&root.join(MARKER_FILE), &foreign).unwrap();
        assert!(super::super::payload::payload_ready(descriptor, &root));
        assert!(!super::super::receipt::stored_identity_matches_descriptor(
            descriptor,
            &super::super::receipt::ParsedStoredReceipt::V1(foreign.clone()),
        ));
        let _ = std::fs::remove_dir_all(root);
    }
}
