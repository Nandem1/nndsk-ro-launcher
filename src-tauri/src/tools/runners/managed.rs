use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::tools::artifacts::{
    self, artifact_ready, catalog_descriptor, decode_hex_digest, ensure_catalog_artifact,
    runtime_dir, DXVK_SHA256, UMU_SHA256,
};
use crate::utils::{emit_log, OperationGuard};

pub const MANAGED_RUNNER_ID: &str = artifacts::MANAGED_RUNNER_ID;
pub const MANAGED_RUNNER_LABEL: &str = artifacts::MANAGED_RUNNER_LABEL;
pub(crate) const LEGACY_MANAGED_RUNNER_ID: &str = artifacts::LEGACY_MANAGED_RUNNER_ID;
pub(crate) const LEGACY_MANAGED_RUNNER_LABEL: &str = artifacts::LEGACY_MANAGED_RUNNER_LABEL;
pub(crate) const LOCAL_MANAGED_RUNNER_ID: &str = artifacts::LOCAL_MANAGED_RUNNER_ID;
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

pub(crate) fn managed_proton_path_for_id(id: &str) -> Option<PathBuf> {
    [
        MANAGED_RUNNER_ID,
        LOCAL_MANAGED_RUNNER_ID,
        LEGACY_MANAGED_RUNNER_ID,
    ]
    .contains(&id)
    .then(|| managed_runtime_dir().join(id).join("proton"))
}

/// Preserve the identity of existing installations instead of relabelling old prefixes.
pub(crate) fn managed_proton_id_for_path(path: &Path) -> Option<&'static str> {
    let canonical =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    [
        MANAGED_RUNNER_ID,
        LOCAL_MANAGED_RUNNER_ID,
        LEGACY_MANAGED_RUNNER_ID,
    ]
    .into_iter()
    .find(|id| canonical(path) == canonical(&managed_proton_path_for_id(id).unwrap()))
}

pub(crate) fn managed_proton_ready(id: &str) -> bool {
    managed_proton_path_for_id(id).is_some() && catalog_descriptor(id).is_some_and(artifact_ready)
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
    managed_runtime_ready_for_id(MANAGED_RUNNER_ID)
}

pub(crate) fn managed_runtime_ready_for_id(id: &str) -> bool {
    catalog_descriptor(UMU_ID).is_some_and(artifact_ready) && managed_proton_ready(id)
}

pub fn managed_dxvk_source_digest_bytes() -> Vec<u8> {
    decode_hex_digest(DXVK_SHA256)
}

pub fn managed_umu_source_digest_bytes() -> Vec<u8> {
    decode_hex_digest(UMU_SHA256)
}

/// Provision only the effective runner. A Wine 7.16 override must not depend on
/// downloading or importing the product's Proton default.
pub(crate) async fn ensure_selected_runtime(
    app: &AppHandle,
    server_runner: Option<&str>,
    default_runner: Option<&str>,
) -> Result<(), String> {
    match selected_runtime_requirement(server_runner, default_runner) {
        RuntimeRequirement::Managed(id) => ensure_runtime_for_id(app, Some(id)).await,
        RuntimeRequirement::UmuOnly => ensure_runtime_for_id(app, None).await,
        RuntimeRequirement::None => Ok(()),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RuntimeRequirement {
    Managed(&'static str),
    UmuOnly,
    None,
}

fn selected_runtime_requirement(
    server_runner: Option<&str>,
    default_runner: Option<&str>,
) -> RuntimeRequirement {
    let selected = server_runner
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            default_runner
                .map(str::trim)
                .filter(|value| !value.is_empty())
        });
    let id = selected
        .map(|path| managed_proton_id_for_path(Path::new(path)))
        .unwrap_or(Some(MANAGED_RUNNER_ID));
    if let Some(id) = id {
        return RuntimeRequirement::Managed(id);
    }
    // An external Proton still needs managed UMU, but not the default Proton.
    if selected.is_some_and(|path| {
        Path::new(path)
            .file_name()
            .is_some_and(|name| name == "proton")
    }) {
        return RuntimeRequirement::UmuOnly;
    }
    RuntimeRequirement::None
}

async fn ensure_runtime_for_id(app: &AppHandle, id: Option<&str>) -> Result<(), String> {
    let runtime_dir = managed_runtime_dir();
    std::fs::create_dir_all(&runtime_dir).map_err(|error| {
        format!(
            "No se pudo crear el directorio del runtime {}: {error}",
            runtime_dir.display()
        )
    })?;
    let operation = OperationGuard::acquire("runtime", &runtime_dir)?;

    ensure_catalog_artifact(app, catalog_descriptor(UMU_ID).unwrap(), 1, 5).await?;
    if let Some(id) = id {
        let descriptor = catalog_descriptor(id).expect("known Proton descriptor");
        if descriptor.kind == artifacts::ArtifactKind::NndskRoProton {
            artifacts::download_managed_runtime(app, descriptor, operation, 6, 38).await?;
        } else {
            ensure_catalog_artifact(app, descriptor, 6, 38).await?;
        }
        emit_log(
            app,
            format!(
                "Runtime Ragnarok listo: {}",
                managed_runtime_dir().join(id).display()
            ),
        )?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_wine_override_needs_no_default_proton() {
        let default = managed_proton_path();
        assert_eq!(
            selected_runtime_requirement(
                Some("/portable/wine-7.16/bin/wine"),
                Some(default.to_str().unwrap())
            ),
            RuntimeRequirement::None
        );
        assert_eq!(
            selected_runtime_requirement(None, Some("/custom/bin/wine")),
            RuntimeRequirement::None
        );
        assert_eq!(
            selected_runtime_requirement(Some("  "), Some("/custom/bin/wine")),
            RuntimeRequirement::None
        );
    }

    #[test]
    fn default_and_legacy_keep_separate_provisioning_identities() {
        let new = managed_proton_path();
        let old = managed_proton_path_for_id(LEGACY_MANAGED_RUNNER_ID).unwrap();
        assert_eq!(
            selected_runtime_requirement(None, None),
            RuntimeRequirement::Managed(MANAGED_RUNNER_ID)
        );
        assert_eq!(
            selected_runtime_requirement(None, Some(new.to_str().unwrap())),
            RuntimeRequirement::Managed(MANAGED_RUNNER_ID)
        );
        assert_eq!(
            selected_runtime_requirement(Some(old.to_str().unwrap()), Some(new.to_str().unwrap())),
            RuntimeRequirement::Managed(LEGACY_MANAGED_RUNNER_ID)
        );
        assert_eq!(
            selected_runtime_requirement(Some("/external/Proton/proton"), None),
            RuntimeRequirement::UmuOnly
        );
        assert!(managed_proton_path_for_id("unknown").is_none());
    }

    #[test]
    fn publication_never_migrates_a_preservation_runner_selection() {
        let local = managed_proton_path_for_id(LOCAL_MANAGED_RUNNER_ID).unwrap();
        assert_ne!(local, managed_proton_path());
        assert_eq!(
            managed_proton_id_for_path(&local),
            Some(LOCAL_MANAGED_RUNNER_ID)
        );
        assert_eq!(
            selected_runtime_requirement(None, Some(local.to_str().unwrap())),
            RuntimeRequirement::Managed(LOCAL_MANAGED_RUNNER_ID)
        );
        assert_eq!(
            selected_runtime_requirement(
                Some(local.to_str().unwrap()),
                Some(managed_proton_path().to_str().unwrap())
            ),
            RuntimeRequirement::Managed(LOCAL_MANAGED_RUNNER_ID)
        );
    }
}
