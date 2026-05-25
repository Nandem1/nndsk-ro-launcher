use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn find_file_case_insensitive(dir: &Path, filename: &str) -> Option<PathBuf> {
    let target = filename.to_ascii_lowercase();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(&target))
        {
            return Some(path);
        }
    }
    None
}

pub fn find_matching_exe(dir: &Path, predicate: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut matches: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| predicate(&n.to_ascii_lowercase()))
        })
        .collect();

    matches.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    matches.into_iter().next()
}

pub fn normalize_token(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("desconocido")
        .to_string()
}

pub fn ensure_dir_writable(dir: &Path) {
    if let Ok(meta) = std::fs::metadata(dir) {
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(dir, perms);
    }
}
