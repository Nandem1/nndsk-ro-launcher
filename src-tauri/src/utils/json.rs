use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonLoadStatus {
    Unchanged,
    Migrated,
    Recovered,
}

#[derive(Debug)]
pub struct JsonLoad<T> {
    pub value: T,
    pub status: JsonLoadStatus,
}

pub fn load_json_recovering<T, F>(path: &Path, validate: F) -> Result<JsonLoad<T>, String>
where
    T: DeserializeOwned + Serialize + Default,
    F: Fn(&T) -> Result<(), String>,
{
    if !path.exists() {
        return Ok(JsonLoad {
            value: T::default(),
            status: JsonLoadStatus::Unchanged,
        });
    }

    match read_validated(path, &validate) {
        Ok((value, original, canonical)) => {
            let status = if original == canonical {
                JsonLoadStatus::Unchanged
            } else {
                write_json(path, &value)?;
                JsonLoadStatus::Migrated
            };
            Ok(JsonLoad { value, status })
        }
        Err(primary_error) => {
            let backup = backup_path(path);
            let (value, _, _) = read_validated(&backup, &validate).map_err(|backup_error| {
                format!(
                    "No se pudo cargar {} ({primary_error}) ni su backup {} ({backup_error})",
                    path.display(),
                    backup.display()
                )
            })?;

            preserve_corrupt_file(path)?;
            replace_json(path, &value)?;
            Ok(JsonLoad {
                value,
                status: JsonLoadStatus::Recovered,
            })
        }
    }
}

fn read_validated<T, F>(
    path: &Path,
    validate: &F,
) -> Result<(T, serde_json::Value, serde_json::Value), String>
where
    T: DeserializeOwned + Serialize,
    F: Fn(&T) -> Result<(), String>,
{
    let content = fs::read_to_string(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let original: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("JSON inválido en {}: {error}", path.display()))?;
    let value: T = serde_json::from_value(original.clone())
        .map_err(|error| format!("estructura inválida en {}: {error}", path.display()))?;
    validate(&value).map_err(|error| format!("datos inválidos en {}: {error}", path.display()))?;
    let canonical = serde_json::to_value(&value).map_err(|error| error.to_string())?;
    Ok((value, original, canonical))
}

pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    write_json_internal(path, value, true)
}

pub fn replace_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    write_json_internal(path, value, false)
}

fn write_json_internal<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
    rotate_backup: bool,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("La ruta no tiene directorio padre: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temp_path = temp_path(path);

    let result = (|| {
        write_and_sync(&temp_path, json.as_bytes())?;
        if rotate_backup && path.exists() {
            copy_and_sync(path, &backup_path(path))?;
        }
        fs::rename(&temp_path, path).map_err(|error| error.to_string())?;
        sync_directory(parent)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn temp_path(path: &Path) -> PathBuf {
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let filename = file_name(path);
    path.with_file_name(format!(".{filename}.tmp-{}-{sequence}", std::process::id()))
}

pub fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name(format!("{}.bak", file_name(path)))
}

fn preserve_corrupt_file(path: &Path) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let quarantine = path.with_file_name(format!(
        "{}.corrupt-{timestamp}-{}-{sequence}",
        file_name(path),
        std::process::id()
    ));
    copy_and_sync(path, &quarantine)?;
    Ok(quarantine)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data")
        .to_string()
}

fn copy_and_sync(source: &Path, destination: &Path) -> Result<(), String> {
    fs::copy(source, destination).map_err(|error| error.to_string())?;
    File::open(destination)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())
}

fn write_and_sync(path: &Path, content: &[u8]) -> Result<(), String> {
    let mut file: File = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(content).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
    struct Fixture {
        #[serde(default)]
        name: String,
        #[serde(default = "default_count")]
        count: u32,
    }

    fn default_count() -> u32 {
        1
    }

    fn test_path(name: &str) -> PathBuf {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "ro-launcher-json-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.join("config.json")
    }

    fn validate(value: &Fixture) -> Result<(), String> {
        if value.name == "invalid" {
            Err("nombre inválido".into())
        } else {
            Ok(())
        }
    }

    #[test]
    fn writes_atomically_and_keeps_the_previous_version_as_backup() {
        let path = test_path("write");
        write_json(&path, &vec!["first"]).unwrap();
        write_json(&path, &vec!["second"]).unwrap();

        assert_eq!(
            serde_json::from_str::<Vec<String>>(&fs::read_to_string(&path).unwrap()).unwrap(),
            vec!["second"]
        );
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&fs::read_to_string(backup_path(&path)).unwrap())
                .unwrap(),
            vec!["first"]
        );
    }

    #[test]
    fn missing_file_returns_defaults_without_creating_it() {
        let path = test_path("missing");
        let loaded = load_json_recovering(&path, validate).unwrap();
        assert_eq!(loaded.value, Fixture::default());
        assert_eq!(loaded.status, JsonLoadStatus::Unchanged);
        assert!(!path.exists());
    }

    #[test]
    fn rewrites_non_canonical_json_and_preserves_the_original() {
        let path = test_path("migrate");
        fs::write(&path, r#"{"name":"old"}"#).unwrap();

        let loaded = load_json_recovering(&path, validate).unwrap();
        assert_eq!(loaded.status, JsonLoadStatus::Migrated);
        assert_eq!(loaded.value.count, 1);
        assert_eq!(
            fs::read_to_string(backup_path(&path)).unwrap(),
            r#"{"name":"old"}"#
        );
        assert_eq!(
            serde_json::from_str::<Fixture>(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn canonical_json_is_not_rewritten() {
        let path = test_path("canonical");
        fs::write(&path, r#"{"count":2,"name":"current"}"#).unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();

        let loaded = load_json_recovering(&path, validate).unwrap();
        assert_eq!(loaded.status, JsonLoadStatus::Unchanged);
        assert!(!backup_path(&path).exists());
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn failed_backup_rotation_leaves_the_primary_untouched() {
        let path = test_path("failed-write");
        fs::write(&path, r#"{"name":"original","count":1}"#).unwrap();
        fs::create_dir(backup_path(&path)).unwrap();

        let error = write_json(
            &path,
            &Fixture {
                name: "replacement".into(),
                count: 2,
            },
        )
        .unwrap_err();
        assert!(!error.is_empty());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"name":"original","count":1}"#
        );
    }

    #[test]
    fn recovers_from_valid_backup_without_overwriting_it() {
        let path = test_path("recover");
        let backup = backup_path(&path);
        fs::write(&path, "not-json").unwrap();
        fs::write(&backup, r#"{"name":"backup","count":2}"#).unwrap();

        let loaded = load_json_recovering(&path, validate).unwrap();
        assert_eq!(loaded.status, JsonLoadStatus::Recovered);
        assert_eq!(loaded.value.name, "backup");
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            r#"{"name":"backup","count":2}"#
        );
        let prefix = "config.json.corrupt-";
        assert!(path
            .parent()
            .unwrap()
            .read_dir()
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(prefix)));
    }

    #[test]
    fn leaves_invalid_primary_untouched_when_backup_is_invalid() {
        let path = test_path("invalid-backup");
        fs::write(&path, "primary-broken").unwrap();
        fs::write(backup_path(&path), "backup-broken").unwrap();

        let error = load_json_recovering::<Fixture, _>(&path, validate).unwrap_err();
        assert!(error.contains("ni su backup"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "primary-broken");
    }

    #[test]
    fn validation_failure_uses_a_valid_backup() {
        let path = test_path("invalid-data");
        fs::write(&path, r#"{"name":"invalid","count":1}"#).unwrap();
        fs::write(backup_path(&path), r#"{"name":"restored","count":3}"#).unwrap();

        let loaded = load_json_recovering(&path, validate).unwrap();
        assert_eq!(loaded.status, JsonLoadStatus::Recovered);
        assert_eq!(loaded.value.name, "restored");
    }
}
