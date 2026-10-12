use crate::{
    models::{
        installation::{CreateInstallation, InstallationDraft},
        server::ServerConfig,
    },
    state::{GameState, ServerRepository, StorageNotices},
    tools::installations,
};
use tauri::{AppHandle, State};

#[tauri::command]
pub fn list_installations(state: State<'_, GameState>) -> Result<Vec<InstallationDraft>, String> {
    state.installations.list()
}

#[tauri::command]
pub fn create_installation(
    state: State<'_, GameState>,
    input: CreateInstallation,
) -> Result<InstallationDraft, String> {
    state.installations.create(input)
}

#[tauri::command]
pub fn start_installation(
    app: AppHandle,
    state: State<'_, GameState>,
    id: String,
) -> Result<InstallationDraft, String> {
    installations::start(
        state.installations.clone(),
        app,
        state.game.clone(),
        state.sessions.clone(),
        &id,
    )
}

#[tauri::command]
pub fn cancel_installation(
    state: State<'_, GameState>,
    id: String,
) -> Result<InstallationDraft, String> {
    state.installations.cancel(&id)
}

#[tauri::command]
pub fn scan_installation_candidates(
    state: State<'_, GameState>,
    id: String,
) -> Result<Vec<String>, String> {
    installations::scan_candidates(&state.installations, &id)
}

#[tauri::command]
pub async fn confirm_installation(
    state: State<'_, GameState>,
    servers: State<'_, ServerRepository>,
    notices: State<'_, StorageNotices>,
    id: String,
    executable_path: String,
    patcher_path: Option<String>,
) -> Result<ServerConfig, String> {
    installations::confirm(
        &state.installations,
        &servers,
        &notices,
        &id,
        executable_path,
        patcher_path,
    )
    .await
}
