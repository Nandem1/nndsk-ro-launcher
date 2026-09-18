use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::models::settings::AppSettings;
use crate::utils::{data_file, load_json_recovering, write_json, JsonLoadStatus};

static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

pub fn settings_path() -> PathBuf {
    data_file("settings.json")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsDocumentOrigin {
    Missing,
    Persisted,
    Recovered,
}

#[derive(Debug)]
pub struct SettingsDocumentLoad {
    pub value: AppSettings,
    pub status: JsonLoadStatus,
    pub origin: SettingsDocumentOrigin,
}

pub fn load_settings_document() -> Result<SettingsDocumentLoad, String> {
    load_settings_document_at(&settings_path())
}

fn load_settings_document_at(path: &Path) -> Result<SettingsDocumentLoad, String> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "El repositorio de configuración está bloqueado".to_string())?;
    let existed = path.exists();
    let loaded = load_json_recovering(path, AppSettings::validate)?;
    let origin = if !existed {
        SettingsDocumentOrigin::Missing
    } else if loaded.status == JsonLoadStatus::Recovered {
        SettingsDocumentOrigin::Recovered
    } else {
        SettingsDocumentOrigin::Persisted
    };
    Ok(SettingsDocumentLoad {
        value: loaded.value,
        status: loaded.status,
        origin,
    })
}

pub fn save_settings_document(settings: &AppSettings) -> Result<(), String> {
    settings.validate()?;
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "El repositorio de configuración está bloqueado".to_string())?;
    write_json(&settings_path(), settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::backup_path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_path(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "ro-launcher-settings-origin-{label}-{}-{}",
                std::process::id(),
                TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ))
            .join("settings.json")
    }

    #[test]
    fn distinguishes_missing_persisted_and_recovered_documents() {
        let missing_path = test_path("missing");
        let missing = load_settings_document_at(&missing_path).unwrap();
        assert_eq!(missing.origin, SettingsDocumentOrigin::Missing);
        assert!(!missing_path.exists());

        let persisted_path = test_path("persisted");
        std::fs::create_dir_all(persisted_path.parent().unwrap()).unwrap();
        std::fs::write(
            &persisted_path,
            r#"{"defaultRunner":"/opt/portable/bin/wine"}"#,
        )
        .unwrap();
        let persisted = load_settings_document_at(&persisted_path).unwrap();
        assert_eq!(persisted.origin, SettingsDocumentOrigin::Persisted);
        assert_eq!(persisted.value.default_runner, "/opt/portable/bin/wine");

        let recovered_path = test_path("recovered");
        std::fs::create_dir_all(recovered_path.parent().unwrap()).unwrap();
        std::fs::write(&recovered_path, "broken").unwrap();
        std::fs::write(
            backup_path(&recovered_path),
            r#"{"defaultRunner":"/usr/bin/wine"}"#,
        )
        .unwrap();
        let recovered = load_settings_document_at(&recovered_path).unwrap();
        assert_eq!(recovered.origin, SettingsDocumentOrigin::Recovered);
        assert_eq!(recovered.value.default_runner, "/usr/bin/wine");

        for path in [missing_path, persisted_path, recovered_path] {
            if let Some(parent) = path.parent().filter(|parent| parent.exists()) {
                std::fs::remove_dir_all(parent).unwrap();
            }
        }
    }
}
