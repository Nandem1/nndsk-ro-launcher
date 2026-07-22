use crate::models::{autobuff::AutobuffStatusEvent, server::ServerConfig};
use crate::state::GameState;
use crate::tools::autobuff::start_session;
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn start_autobuff(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    let _tool_lifecycle = state.tool_lifecycle.lock().await;
    server.validate_executable_available()?;
    let launcher_pid = state.game.sole_running_pid_for(&server.id)?;
    start_session(
        app,
        &state.autobuff,
        state.input.clone(),
        launcher_pid,
        server,
    )
    .await
}
#[tauri::command]
pub async fn stop_autobuff(state: State<'_, GameState>) -> Result<(), String> {
    let _tool_lifecycle = state.tool_lifecycle.lock().await;
    state.autobuff.stop().await
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
