use crate::models::{autobuff::AutobuffStatusEvent, server::ServerConfig};
use crate::state::GameState;
use crate::tools::autobuff::start_session;
use std::sync::Arc;
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn start_autobuff(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    server.validate_executable_available()?;
    let launcher_pid = *state
        .pid
        .lock()
        .unwrap()
        .as_ref()
        .ok_or_else(|| "No hay proceso Wine del juego (lanza el juego primero)".to_string())?;
    start_session(
        app,
        &state.autobuff,
        state.input.clone(),
        Arc::clone(&state.ydotoold),
        launcher_pid,
        server,
    )
    .await
}
#[tauri::command]
pub async fn stop_autobuff(state: State<'_, GameState>) -> Result<(), String> {
    state.autobuff.stop().await;
    Ok(())
}
#[tauri::command]
pub fn update_autobuff_config(
    state: State<'_, GameState>,
    config: ro_tools_core::AutobuffConfig,
) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    state.autobuff.update_config(config)
}
#[tauri::command]
pub fn get_autobuff_status(state: State<'_, GameState>) -> AutobuffStatusEvent {
    state.autobuff.status()
}
