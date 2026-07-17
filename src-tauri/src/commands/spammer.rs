use tauri::{AppHandle, State};

use crate::models::server::ServerConfig;
use crate::models::spammer::SpammerStatusEvent;
use crate::state::GameState;
use crate::tools::spammer::start_session;

#[tauri::command]
pub async fn start_spammer(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    server.validate()?;
    state
        .game
        .running_pid()?
        .ok_or_else(|| "No hay juego en ejecución (lanza el juego primero)".to_string())?;

    start_session(
        app,
        &state.spammer,
        state.input.clone(),
        server.spammer.clone(),
    )
    .await
}

#[tauri::command]
pub async fn stop_spammer(state: State<'_, GameState>) -> Result<(), String> {
    state.spammer.stop().await
}

/// Config change = restart completo (no hay canal live; ro-inputd es stateless en config).
#[tauri::command]
pub async fn update_spammer_config(
    app: AppHandle,
    state: State<'_, GameState>,
    config: ro_tools_core::SpammerConfig,
) -> Result<(), String> {
    state
        .game
        .running_pid()?
        .ok_or_else(|| "No hay juego en ejecución (lanza el juego primero)".to_string())?;

    start_session(app, &state.spammer, state.input.clone(), config).await
}

#[tauri::command]
pub fn get_spammer_status(state: State<'_, GameState>) -> SpammerStatusEvent {
    state.spammer.status()
}
