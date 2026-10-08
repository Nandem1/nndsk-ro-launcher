use std::path::{Path, PathBuf};

use crate::models::runner::RunnerInfo;
use crate::tools::{artifacts, runners};

#[tauri::command]
pub async fn list_runners() -> Result<Vec<RunnerInfo>, String> {
    runners::discover_runners()
}

fn validate_archive_path(path: &str) -> Result<PathBuf, String> {
    if path.contains('\0') || !Path::new(path).is_absolute() {
        return Err("Selecciona una ruta absoluta al paquete del runtime".into());
    }
    Ok(PathBuf::from(path))
}

#[tauri::command]
pub async fn import_managed_runtime_archive(archive_path: String) -> Result<String, String> {
    let archive_path = validate_archive_path(&archive_path)?;
    let entrypoint = artifacts::import_managed_runtime_archive(&archive_path).await?;
    Ok(entrypoint.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::validate_archive_path;
    use std::path::PathBuf;

    #[test]
    fn rejects_relative_empty_and_nul_archive_paths() {
        for path in [
            "",
            "runtime.tar.zst",
            "../runtime.tar.zst",
            "/tmp/invalid\0.zst",
        ] {
            assert!(validate_archive_path(path).is_err());
        }
    }

    #[test]
    fn keeps_absolute_archive_paths_without_rewriting_spaces() {
        let path = "/tmp/My Runtime/runtime.tar.zst";
        assert_eq!(validate_archive_path(path).unwrap(), PathBuf::from(path));
    }
}
