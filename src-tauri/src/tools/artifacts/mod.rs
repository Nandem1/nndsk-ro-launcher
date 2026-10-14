//! Catálogo y pipeline de instalación de artefactos managed (Fase 3).

mod descriptor;
mod extract;
mod fetch;
mod import;
mod install;
mod payload;
mod receipt;
mod source;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::utils::{app_data_dir, OperationGuard};

#[allow(unused_imports)]
pub(crate) use descriptor::{
    catalog_all, catalog_descriptor, decode_hex_digest, expected_digest_hex, ArtifactDescriptor,
    ArtifactKind, ArtifactSource, ExpectedDigest, DXVK_SHA256, LEGACY_MANAGED_RUNNER_ID,
    LEGACY_MANAGED_RUNNER_LABEL, LOCAL_MANAGED_RUNNER_ID, MANAGED_DXVK_ID, MANAGED_RUNNER_ID,
    MANAGED_RUNNER_LABEL, MANAGED_WINE716_ID, MANAGED_WINE716_LABEL, NNDSK_RUNTIME_SHA256,
    NNDSK_RUNTIME_SOURCE_COMMIT, PREVIOUS_MANAGED_RUNNER_ID, PROTON_SHA512, UMU_ID, UMU_SHA256,
};
pub(crate) use fetch::{
    download_pinned_https, pinned_https_url_allowed, verify_archive_file, PinnedHttpsFile,
};
pub(crate) use import::download_managed_runtime;
pub(crate) use import::import_managed_runtime_archive;

const RUNTIME_DIR: &str = "runtime";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactCacheState {
    Ready,
    Missing,
    InvalidDirectory,
    InvalidReceipt,
    ReceiptMismatch,
    IncompletePayload,
}

impl ArtifactCacheState {
    fn reason(self) -> &'static str {
        match self {
            Self::Ready => "listo",
            Self::Missing => "instalación ausente",
            Self::InvalidDirectory => "directorio de instalación no válido",
            Self::InvalidReceipt => "recibo ausente o ilegible",
            Self::ReceiptMismatch => "recibo distinto del catálogo actual",
            Self::IncompletePayload => "faltan archivos esenciales",
        }
    }
}

pub(crate) fn runtime_dir() -> PathBuf {
    app_data_dir().join(RUNTIME_DIR)
}

pub(crate) fn runtime_artifact_root(descriptor: &ArtifactDescriptor) -> PathBuf {
    runtime_dir().join(descriptor.id)
}

pub(crate) fn artifact_ready(descriptor: &ArtifactDescriptor) -> bool {
    let root = runtime_artifact_root(descriptor);
    artifact_cache_state_at(descriptor, &root) == ArtifactCacheState::Ready
}

fn artifact_cache_state_at(
    descriptor: &ArtifactDescriptor,
    root: &std::path::Path,
) -> ArtifactCacheState {
    let metadata = match root.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ArtifactCacheState::Missing;
        }
        Err(_) => return ArtifactCacheState::InvalidDirectory,
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return ArtifactCacheState::InvalidDirectory;
    }
    // Pin the directory inode while validating. Exchange plus cleanup can otherwise make a
    // reader mix an old receipt with a new payload, or read old files as they are deleted.
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    loop {
        let Ok(directory) = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(root)
        else {
            return ArtifactCacheState::InvalidDirectory;
        };
        let Ok(pinned) = directory.metadata() else {
            return ArtifactCacheState::InvalidDirectory;
        };
        #[cfg(test)]
        install::checkpoint("reader_pinned").expect("reader rendezvous");
        let handle_path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let state = cache_state_in_directory(descriptor, &handle_path);
        match root.symlink_metadata() {
            Ok(current)
                if current.dev() == pinned.dev()
                    && current.ino() == pinned.ino()
                    && current.is_dir()
                    && !current.file_type().is_symlink() =>
            {
                return state
            }
            Ok(_) => continue, // A generation changed: revalidate the current canonical inode.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return ArtifactCacheState::Missing
            }
            Err(_) => return ArtifactCacheState::InvalidDirectory,
        }
    }
}

fn cache_state_in_directory(
    descriptor: &ArtifactDescriptor,
    root: &std::path::Path,
) -> ArtifactCacheState {
    let Ok(Some(stored)) = receipt::parse_stored_receipt(&root.join(receipt::MARKER_FILE)) else {
        return ArtifactCacheState::InvalidReceipt;
    };
    if !receipt::stored_identity_matches_descriptor(descriptor, &stored) {
        return ArtifactCacheState::ReceiptMismatch;
    }
    if !payload::payload_ready(descriptor, root) {
        return ArtifactCacheState::IncompletePayload;
    }
    ArtifactCacheState::Ready
}

/// Acquires runtime ownership before dispatch; the detached owner retains it through acquisition
/// and blocking extraction. The caller never owns cleanup while another task owns these paths.
pub(crate) async fn ensure_catalog_artifact(
    app: &AppHandle,
    descriptor: &'static ArtifactDescriptor,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    let operation = OperationGuard::acquire("runtime", &runtime_dir())?;
    download_managed_runtime(app, descriptor, operation, progress_start, progress_end).await
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::payload::DXVK_DLLS;
    use crate::utils::OperationGuard;

    #[test]
    fn cache_state_explains_why_dxvk_would_be_downloaded() {
        let descriptor = catalog_descriptor(MANAGED_DXVK_ID).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-artifact-cache-state-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        assert_eq!(
            artifact_cache_state_at(descriptor, &root),
            ArtifactCacheState::Missing
        );

        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(
            artifact_cache_state_at(descriptor, &root),
            ArtifactCacheState::InvalidReceipt
        );
        let marker = root.join(receipt::MARKER_FILE);
        std::fs::write(&marker, b"{").unwrap();
        assert_eq!(
            artifact_cache_state_at(descriptor, &root),
            ArtifactCacheState::InvalidReceipt
        );
        let mut receipt = receipt::receipt_v2_from_descriptor(descriptor);
        receipt.artifact_id = "outdated".to_string();
        std::fs::write(&marker, serde_json::to_vec(&receipt).unwrap()).unwrap();
        assert_eq!(
            artifact_cache_state_at(descriptor, &root),
            ArtifactCacheState::ReceiptMismatch
        );

        receipt.artifact_id = descriptor.id.to_string();
        std::fs::write(&marker, serde_json::to_vec(&receipt).unwrap()).unwrap();
        assert_eq!(
            artifact_cache_state_at(descriptor, &root),
            ArtifactCacheState::IncompletePayload
        );
        for arch in ["x32", "x64"] {
            let directory = root.join(arch);
            std::fs::create_dir_all(&directory).unwrap();
            for dll in DXVK_DLLS {
                std::fs::write(directory.join(dll), b"dxvk").unwrap();
            }
        }
        assert_eq!(
            artifact_cache_state_at(descriptor, &root),
            ArtifactCacheState::Ready
        );
        let symlink = root.with_extension("link");
        std::os::unix::fs::symlink(&root, &symlink).unwrap();
        assert_eq!(
            artifact_cache_state_at(descriptor, &symlink),
            ArtifactCacheState::InvalidDirectory
        );
        std::fs::remove_file(symlink).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_runtime_lock_rejects_second_writer() {
        let path = std::env::temp_dir().join(format!(
            "ro-launcher-runtime-lock-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let first = OperationGuard::acquire("runtime", &path).unwrap();
        assert!(OperationGuard::acquire("runtime", &path).is_err());
        drop(first);
        assert!(OperationGuard::acquire("runtime", &path).is_ok());
    }

    #[test]
    fn pinned_proton_is_newer_than_the_broken_dxvk_snapshot() {
        assert_eq!(
            LEGACY_MANAGED_RUNNER_ID,
            "ro-proton-cachyos-11.0-20260702-slr"
        );
        assert_eq!(
            LEGACY_MANAGED_RUNNER_LABEL,
            "proton-cachyos-11.0-20260702-slr-x86_64"
        );
        assert_eq!(PROTON_SHA512.len(), 128);
        assert_eq!(UMU_SHA256.len(), 64);
        assert_eq!(DXVK_SHA256.len(), 64);
        assert_eq!(
            catalog_descriptor(MANAGED_DXVK_ID).unwrap().id,
            "dxvk-2.6.2"
        );
    }
}
