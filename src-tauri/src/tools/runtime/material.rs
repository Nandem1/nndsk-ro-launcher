use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;

#[derive(Debug, Clone, Default)]
pub(crate) struct FileDigestCache {
    digests: HashMap<PathBuf, Option<Vec<u8>>>,
}

impl FileDigestCache {
    pub(crate) fn sha256_file(&mut self, path: &Path) -> Option<Vec<u8>> {
        let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(cached) = self.digests.get(&key) {
            return cached.clone();
        }
        let digest = read_sha256(&key);
        self.digests.insert(key, digest.clone());
        digest
    }
}

fn read_sha256(path: &Path) -> Option<Vec<u8>> {
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    Some(Sha256::digest(bytes).to_vec())
}

#[cfg(target_os = "linux")]
pub(crate) fn file_cache_key(path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let metadata = std::fs::metadata(&canonical).ok()?;
    Some(format!(
        "{}:{}:{}:{}:{}:{}",
        canonical.display(),
        metadata.st_dev(),
        metadata.st_ino(),
        metadata.len(),
        metadata.st_mtime(),
        metadata.st_ctime()
    ))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn file_cache_key(path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let metadata = std::fs::metadata(&canonical).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .map(|time| format!("{time:?}"))
        .unwrap_or_default();
    Ok(format!(
        "{}:{}:{}",
        canonical.display(),
        metadata.len(),
        modified
    ))
}
