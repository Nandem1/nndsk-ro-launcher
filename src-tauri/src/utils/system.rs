use std::path::{Path, PathBuf};

pub const SYSTEM_WINE_CANDIDATES: &[(&str, &str)] = &[
    ("/usr/bin/wine-cachyos", "Wine CachyOS"),
    ("/usr/bin/wine", "Wine"),
    ("/usr/bin/wine64", "Wine64"),
];

pub const WINETRICKS_BIN: &str = "/usr/bin/winetricks";
pub const UMU_RUN_BIN: &str = "/usr/bin/umu-run";

pub fn default_system_wine() -> String {
    discovered_system_wines()
        .into_iter()
        .next()
        .map(|(path, _)| path.to_string_lossy().to_string())
        .unwrap_or_else(|| SYSTEM_WINE_CANDIDATES[1].0.to_string())
}

pub fn discovered_system_wines() -> Vec<(PathBuf, &'static str)> {
    let mut candidates = Vec::new();
    for (path, label) in SYSTEM_WINE_CANDIDATES {
        let path = PathBuf::from(path);
        if is_executable_file(&path) {
            candidates.push((path, *label));
        }
    }
    for (name, label) in [
        ("wine-cachyos", "Wine CachyOS"),
        ("wine", "Wine"),
        ("wine64", "Wine64"),
    ] {
        if let Some(path) = executable_in_path(name) {
            candidates.push((path, label));
        }
    }
    candidates
}

pub fn winetricks_path() -> Option<PathBuf> {
    preferred_or_path(Path::new(WINETRICKS_BIN), "winetricks")
}

pub fn find_umu_run() -> Option<PathBuf> {
    preferred_or_path(Path::new(UMU_RUN_BIN), "umu-run")
}

pub fn winetricks_available() -> bool {
    winetricks_path().is_some()
}

pub fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn preferred_or_path(preferred: &Path, name: &str) -> Option<PathBuf> {
    is_executable_file(preferred)
        .then(|| preferred.to_path_buf())
        .or_else(|| executable_in_path(name))
}

fn executable_in_path(name: &str) -> Option<PathBuf> {
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    directories.push(PathBuf::from("/usr/local/bin"));
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/bin"));
    }

    directories
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|path| is_executable_file(path))
        .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
}
