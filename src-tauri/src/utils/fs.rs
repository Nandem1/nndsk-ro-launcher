use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Makes a completed entry change in `directory` (create, rename, unlink) durable on Linux.
/// A rename is atomic for concurrent observers immediately, but it is only persisted once the
/// containing directory has been synced.
pub fn sync_dir(directory: &Path) -> std::io::Result<()> {
    File::open(directory)?.sync_all()
}

/// Flushes every dirty page of the filesystem containing `path` (syncfs(2)).
pub fn sync_filesystem(path: &Path) -> std::io::Result<()> {
    let handle = File::open(path)?;
    if unsafe { libc::syncfs(handle.as_raw_fd()) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// `rename(2)` that fails with `AlreadyExists` instead of replacing the destination. A plain
/// rename silently replaces an empty destination directory, which would hide a lost race.
pub fn rename_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

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
                && p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
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

pub fn find_bundled_sidecar_in(directory: &Path, stem: &str) -> Option<PathBuf> {
    let exact = directory.join(stem);
    if exact.is_file() {
        return Some(exact);
    }
    let prefix = format!("{stem}-");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return None;
    };
    let mut matches: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == stem || name.starts_with(&prefix))
        })
        .collect();
    matches.sort();
    matches.into_iter().next()
}

pub fn bundled_sidecar_path_from(stem: &str, directory: Option<&Path>) -> PathBuf {
    if let Some(dir) = directory {
        return find_bundled_sidecar_in(dir, stem).unwrap_or_else(|| dir.join(stem));
    }
    PathBuf::from(format!("/nonexistent/ro-launcher/{stem}"))
}

pub fn bundled_sidecar_path(stem: &str) -> PathBuf {
    let directory = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    bundled_sidecar_path_from(stem, directory.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ro-launcher-sidecar-{}-{}",
            std::process::id(),
            suffix
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rename_noreplace_never_replaces_an_empty_directory() {
        let dir = unique_dir();
        let source = dir.join("source");
        let destination = dir.join("destination");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("payload"), b"old").unwrap();
        std::fs::create_dir(&destination).unwrap();
        let error = rename_noreplace(&source, &destination).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(source.join("payload")).unwrap(), b"old");
        std::fs::remove_dir(&destination).unwrap();
        rename_noreplace(&source, &destination).unwrap();
        sync_dir(&dir).unwrap();
        sync_filesystem(&dir).unwrap();
        assert_eq!(std::fs::read(destination.join("payload")).unwrap(), b"old");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bundled_sidecar_never_falls_back_to_path() {
        let dir = unique_dir();
        let missing = bundled_sidecar_path_from("ro-inputd", Some(&dir));
        assert!(missing.is_absolute());
        assert_eq!(missing, dir.join("ro-inputd"));
        assert!(!missing.as_os_str().eq("ro-inputd"));

        let no_exe_dir = bundled_sidecar_path_from("ro-sessiond", None);
        assert!(no_exe_dir.is_absolute());
        assert_eq!(
            no_exe_dir,
            PathBuf::from("/nonexistent/ro-launcher/ro-sessiond")
        );

        let binary = dir.join("ro-sessiond-x86_64-unknown-linux-gnu");
        std::fs::write(&binary, b"sidecar").unwrap();
        let mut perms = std::fs::metadata(&binary).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary, perms).unwrap();
        let found = find_bundled_sidecar_in(&dir, "ro-sessiond").unwrap();
        assert_eq!(found, binary);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
