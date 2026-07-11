use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::launcher;

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    server.validate_executable_available()?;
    launcher::launch_game(
        app,
        &Arc::clone(&state.pid),
        &state.autopot,
        &state.autobuff,
        &state.spammer,
        server,
    )
    .await
}

#[tauri::command]
pub async fn stop_game(state: State<'_, GameState>) -> Result<(), String> {
    launcher::stop_game(&state).await
}
