use std::path::{Path, PathBuf};

use ro_tools_core::dgvoodoo::TEMPLATE_FILES;
use tauri::{AppHandle, Manager};

use crate::utils::find_file_case_insensitive;

pub fn template_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("dgvoodoo");
        if template_is_complete(&bundled) {
            return Ok(bundled);
        }
    }

    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dgvoodoo");
    if template_is_complete(&dev) {
        return Ok(dev);
    }

    Err("Plantilla dgVoodoo no encontrada en el launcher".to_string())
}

pub fn template_is_complete(dir: &Path) -> bool {
    dir.is_dir()
        && TEMPLATE_FILES
            .iter()
            .all(|file| dir.join(file).is_file())
}

pub fn install_files(app: &AppHandle, game_dir: &Path) -> Result<Vec<String>, String> {
    let template_dir = template_dir(app)?;
    let mut installed = Vec::new();

    for file in TEMPLATE_FILES {
        if find_file_case_insensitive(game_dir, file).is_some() {
            continue;
        }

        let source = template_dir.join(file);
        let dest = game_dir.join(file);
        std::fs::copy(&source, &dest)
            .map_err(|e| format!("No se pudo copiar {file}: {e}"))?;
        installed.push((*file).to_string());
    }

    Ok(installed)
}

pub fn uninstall_files(game_dir: &Path) -> Result<Vec<String>, String> {
    let mut removed = Vec::new();

    for file in TEMPLATE_FILES {
        let Some(path) = find_file_case_insensitive(game_dir, file) else {
            continue;
        };

        std::fs::remove_file(&path).map_err(|e| {
            format!(
                "No se pudo eliminar {}: {e}",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(file)
            )
        })?;
        removed.push(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file)
                .to_string(),
        );
    }

    if removed.is_empty() {
        return Err("No hay archivos dgVoodoo para desinstalar".to_string());
    }

    Ok(removed)
}
