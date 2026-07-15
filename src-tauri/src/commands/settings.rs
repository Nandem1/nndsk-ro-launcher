use tauri::State;

use crate::models::settings::AppSettings;
use crate::state::{SettingsRepository, StorageNotices};

#[tauri::command]
pub async fn load_settings(
    repository: State<'_, SettingsRepository>,
    notices: State<'_, StorageNotices>,
) -> Result<AppSettings, String> {
    repository.load(&notices)
}

#[tauri::command]
pub async fn save_settings(
    repository: State<'_, SettingsRepository>,
    settings: AppSettings,
) -> Result<(), String> {
    repository.save(&settings)
}
