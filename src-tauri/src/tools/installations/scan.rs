use std::path::Path;

use super::InstallationManager;
use crate::models::installation::{InstallationDraft, InstallationStatus};

/// Search only user-selected game data and this installation's C drive. Never follow
/// directory symlinks into Z:, other prefixes, or personal folders.
pub fn scan_candidates(manager: &InstallationManager, id: &str) -> Result<Vec<String>, String> {
    let draft = manager.get(id)?;
    if !matches!(
        draft.status,
        InstallationStatus::AwaitingClient | InstallationStatus::Completed
    ) || manager.is_active(id)?
    {
        return Err("Espera a que termine correctamente el instalador".into());
    }
    Ok(candidates(&draft))
}

pub(crate) fn candidates(draft: &InstallationDraft) -> Vec<String> {
    let mut pending = draft
        .destination_path
        .as_ref()
        .and_then(|path| std::fs::canonicalize(path).ok())
        .map(|path| vec![(path, 0)])
        .unwrap_or_default();
    if let Some(binding) = &draft.binding {
        let drive = Path::new(&binding.prefix_path).join("drive_c");
        if drive
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        {
            pending.push((drive, 0));
        }
    }
    let installer = std::fs::canonicalize(&draft.installer_path).ok();
    let mut result = Vec::new();
    let mut visited = 0;
    while let Some((dir, depth)) = pending.pop() {
        if depth > 8 || visited >= 10_000 || result.len() >= 100 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > 10_000 {
                break;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case("windows"))
                {
                    continue;
                }
                pending.push((path, depth + 1));
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            {
                if installer.is_some() && std::fs::canonicalize(&path).ok() == installer {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.starts_with("unins") || name == "setup.exe" {
                    continue;
                }
                result.push(path.to_string_lossy().to_string());
                if result.len() >= 100 {
                    break;
                }
            }
        }
    }
    result.sort();
    result.dedup();
    result
}
