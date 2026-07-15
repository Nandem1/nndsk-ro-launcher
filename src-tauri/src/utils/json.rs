use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    if !path.exists() {
        return Ok(T::default());
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let temp_path = temp_path(path);

    write_and_sync(&temp_path, json.as_bytes())?;

    if path.exists() {
        fs::copy(path, backup_path(path)).map_err(|e| e.to_string())?;
    }

    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        e.to_string()
    })
}

fn temp_path(path: &Path) -> PathBuf {
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data");
    path.with_file_name(format!(".{filename}.tmp-{}-{sequence}", std::process::id()))
}

fn backup_path(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data");
    path.with_file_name(format!("{filename}.bak"))
}

fn write_and_sync(path: &Path, content: &[u8]) -> Result<(), String> {
    let mut file: File = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(content).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_atomically_and_keeps_the_previous_version_as_backup() {
        let path =
            std::env::temp_dir().join(format!("ro-launcher-json-test-{}.json", std::process::id()));
        let backup = backup_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);

        write_json(&path, &vec!["first"]).unwrap();
        write_json(&path, &vec!["second"]).unwrap();

        assert_eq!(read_json::<Vec<String>>(&path).unwrap(), vec!["second"]);
        assert_eq!(read_json::<Vec<String>>(&backup).unwrap(), vec!["first"]);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
    }
}
