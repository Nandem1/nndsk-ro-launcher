use tauri::State;

use crate::models::settings::AppSettings;
use crate::state::{GameState, SettingsRepository, StorageNotices};

#[tauri::command]
pub async fn load_settings(
    repository: State<'_, SettingsRepository>,
    notices: State<'_, StorageNotices>,
    state: State<'_, GameState>,
) -> Result<AppSettings, String> {
    let settings = repository.load(&notices)?;
    state.presence.set_enabled(settings.rich_presence_enabled);
    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(
    repository: State<'_, SettingsRepository>,
    state: State<'_, GameState>,
    settings: AppSettings,
) -> Result<(), String> {
    repository.save(&settings)?;
    state.presence.set_enabled(settings.rich_presence_enabled);
    Ok(())
}
