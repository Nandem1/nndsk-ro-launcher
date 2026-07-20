use std::{path::PathBuf, sync::Mutex};

use crate::models::settings::AppSettings;
use crate::utils::{data_file, load_json_recovering, write_json, JsonLoad};

static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

pub fn settings_path() -> PathBuf {
    data_file("settings.json")
}

pub fn load_settings_document() -> Result<JsonLoad<AppSettings>, String> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "El repositorio de configuración está bloqueado".to_string())?;
    load_json_recovering(&settings_path(), AppSettings::validate)
}

pub fn save_settings_document(settings: &AppSettings) -> Result<(), String> {
    settings.validate()?;
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "El repositorio de configuración está bloqueado".to_string())?;
    write_json(&settings_path(), settings)
}
