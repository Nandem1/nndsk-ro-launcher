use tauri::{AppHandle, State};

use crate::models::autopot::AutopotStatusEvent;
use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::autopot::{load_profiles, start_session};

#[tauri::command]
pub async fn start_autopot(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    server.validate_executable_available()?;
    let launcher_pid = state
        .game
        .running_pid()?
        .ok_or_else(|| "No hay proceso Wine del juego (lanza el juego primero)".to_string())?;

    start_session(
        app,
        &state.autopot,
        state.input.clone(),
        launcher_pid,
        server,
    )
    .await
}

#[tauri::command]
pub async fn stop_autopot(state: State<'_, GameState>) -> Result<(), String> {
    state.autopot.stop().await
}

#[tauri::command]
pub fn update_autopot_config(
    state: State<'_, GameState>,
    config: ro_tools_core::AutopotConfig,
) -> Result<(), String> {
    config.validate().map_err(|error| error.to_string())?;
    state.autopot.update_config(config)
}

#[tauri::command]
pub fn get_autopot_status(state: State<'_, GameState>) -> AutopotStatusEvent {
    state.autopot.status()
}

#[tauri::command]
pub fn list_client_profiles() -> Vec<ro_tools_core::ClientProfile> {
    load_profiles()
}
