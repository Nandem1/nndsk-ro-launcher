use std::path::PathBuf;

use tauri::AppHandle;

use crate::tools::artifacts::{
    self, artifact_ready, catalog_descriptor, decode_hex_digest, ensure_catalog_artifact,
    runtime_dir, DXVK_SHA256, PROTON_SHA512, UMU_SHA256,
};
use crate::utils::{emit_log, OperationGuard};

pub const MANAGED_RUNNER_ID: &str = artifacts::MANAGED_RUNNER_ID;
pub const MANAGED_RUNNER_LABEL: &str = artifacts::MANAGED_RUNNER_LABEL;
pub(crate) const MANAGED_DXVK_ID: &str = artifacts::MANAGED_DXVK_ID;
pub(crate) const UMU_ID: &str = artifacts::UMU_ID;

pub fn managed_runtime_dir() -> PathBuf {
    runtime_dir()
}

pub fn managed_runner_root() -> PathBuf {
    managed_runtime_dir().join(MANAGED_RUNNER_ID)
}

pub fn managed_proton_path() -> PathBuf {
    managed_runner_root().join("proton")
}

pub fn managed_dxvk_root() -> PathBuf {
    managed_runtime_dir().join(MANAGED_DXVK_ID)
}

pub fn managed_dxvk_ready() -> bool {
    catalog_descriptor(MANAGED_DXVK_ID).is_some_and(artifact_ready)
}

pub fn managed_umu_root() -> PathBuf {
    managed_runtime_dir().join(UMU_ID)
}

pub fn managed_umu_path() -> PathBuf {
    managed_umu_root().join("umu-run")
}

pub fn managed_runtime_ready() -> bool {
    catalog_descriptor(UMU_ID).is_some_and(artifact_ready)
        && catalog_descriptor(MANAGED_RUNNER_ID).is_some_and(artifact_ready)
}

pub fn managed_proton_source_digest_bytes() -> Vec<u8> {
    decode_hex_digest(PROTON_SHA512)
}

pub fn managed_dxvk_source_digest_bytes() -> Vec<u8> {
    decode_hex_digest(DXVK_SHA256)
}

pub fn managed_umu_source_digest_bytes() -> Vec<u8> {
    decode_hex_digest(UMU_SHA256)
}

pub async fn ensure_managed_runtime(app: &AppHandle) -> Result<(), String> {
    let runtime_dir = managed_runtime_dir();
    std::fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "No se pudo crear el directorio del runtime {}: {error}",
            runtime_dir.display()
        )
    })?;
    let _operation = OperationGuard::acquire("runtime", &runtime_dir)?;

    ensure_catalog_artifact(app, catalog_descriptor(UMU_ID).unwrap(), 1, 5).await?;
    ensure_catalog_artifact(app, catalog_descriptor(MANAGED_RUNNER_ID).unwrap(), 6, 38).await?;
    emit_log(
        app,
        format!(
            "Runtime Ragnarok listo: {}",
            managed_runner_root().display()
        ),
    )?;
    Ok(())
}

/// Instala DXVK sólo cuando un Wine legacy lo necesita. Los usuarios de Proton no descargan este
/// componente adicional porque Proton ya administra su propia versión.
pub async fn ensure_managed_dxvk(app: &AppHandle) -> Result<(), String> {
    let runtime_dir = managed_runtime_dir();
    std::fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "No se pudo crear el directorio del runtime {}: {error}",
            runtime_dir.display()
        )
    })?;
    let _operation = OperationGuard::acquire("runtime", &runtime_dir)?;
    ensure_catalog_artifact(app, catalog_descriptor(MANAGED_DXVK_ID).unwrap(), 40, 54).await
}
