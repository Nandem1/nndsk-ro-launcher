//! Catálogo y pipeline de instalación de artefactos managed (Fase 3).

mod descriptor;
mod extract;
mod fetch;
mod install;
mod payload;
mod receipt;
mod source;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::utils::{app_data_dir, emit_log, emit_progress, replace_json};

#[allow(unused_imports)]
pub(crate) use descriptor::{
    catalog_all, catalog_descriptor, decode_hex_digest, expected_digest_hex, ArtifactDescriptor,
    ExpectedDigest, DXVK_SHA256, MANAGED_DXVK_ID, MANAGED_RUNNER_ID, MANAGED_RUNNER_LABEL,
    PROTON_SHA512, UMU_ID, UMU_SHA256,
};

const RUNTIME_DIR: &str = "runtime";

pub(crate) fn runtime_dir() -> PathBuf {
    app_data_dir().join(RUNTIME_DIR)
}

pub(crate) fn runtime_artifact_root(descriptor: &ArtifactDescriptor) -> PathBuf {
    runtime_dir().join(descriptor.id)
}

pub(crate) fn artifact_ready(descriptor: &ArtifactDescriptor) -> bool {
    let root = runtime_artifact_root(descriptor);
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return false;
    }
    let marker_path = root.join(receipt::MARKER_FILE);
    let parsed = receipt::parse_stored_receipt(&marker_path).ok().flatten();
    parsed.is_some_and(|stored| {
        receipt::stored_identity_matches_descriptor(descriptor, &stored)
            && payload::payload_ready(descriptor, &root)
    })
}

pub(crate) fn try_elevate_marker_v1_to_v2(
    _app: &AppHandle,
    descriptor: &ArtifactDescriptor,
) -> Result<(), String> {
    let root = runtime_artifact_root(descriptor);
    let marker_path = root.join(receipt::MARKER_FILE);
    let Some(receipt::ParsedStoredReceipt::V1(_)) =
        receipt::parse_stored_receipt(&marker_path).ok().flatten()
    else {
        return Ok(());
    };
    replace_json(
        &marker_path,
        &receipt::receipt_v2_from_descriptor(descriptor),
    )
}

pub(crate) async fn ensure_catalog_artifact(
    app: &AppHandle,
    descriptor: &'static ArtifactDescriptor,
    progress_start: u32,
    progress_end: u32,
) -> Result<(), String> {
    if artifact_ready(descriptor) {
        if let Err(error) = try_elevate_marker_v1_to_v2(app, descriptor) {
            emit_log(
                app,
                format!("No se pudo elevar el receipt de {}: {error}", descriptor.id),
            )?;
        }
        return Ok(());
    }

    let runtime_dir = runtime_dir();
    let final_dir = runtime_dir.join(descriptor.id);
    let unique = unique_suffix();
    let download_path = runtime_dir.join(format!(
        ".{}.download-{}-{unique}",
        descriptor.archive.archive_name(),
        std::process::id()
    ));
    let staging_dir = runtime_dir.join(format!(
        ".{}.staging-{}-{unique}",
        descriptor.id,
        std::process::id()
    ));

    let mut cleanup = install::StagingCleanup::new(download_path.clone(), staging_dir.clone());
    let result = async {
        emit_log(app, format!("Descargando {}...", descriptor.id))?;
        fetch::download_verified(
            app,
            descriptor,
            &download_path,
            progress_start,
            progress_end.saturating_sub(2),
        )
        .await?;
        emit_progress(
            app,
            &format!("Verificando {}...", descriptor.id),
            progress_end - 1,
        )?;

        let descriptor_copy = *descriptor;
        let download_copy = download_path.clone();
        let staging_copy = staging_dir.clone();
        tokio::task::spawn_blocking(move || {
            extract::extract_archive(&descriptor_copy, &download_copy, &staging_copy)
        })
        .await
        .map_err(|error| format!("Falló la tarea de extracción: {error}"))??;

        emit_progress(
            app,
            &format!("Instalando {}...", descriptor.id),
            progress_end,
        )?;
        install::install_extracted(descriptor, &staging_dir, &final_dir, artifact_ready)
    }
    .await;

    cleanup.disarm();
    drop(cleanup);
    let _ = std::fs::remove_file(&download_path);
    let _ = std::fs::remove_dir_all(&staging_dir);
    result
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
    use crate::utils::OperationGuard;

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
        assert_eq!(MANAGED_RUNNER_ID, "ro-proton-cachyos-11.0-20260702-slr");
        assert_eq!(
            MANAGED_RUNNER_LABEL,
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
