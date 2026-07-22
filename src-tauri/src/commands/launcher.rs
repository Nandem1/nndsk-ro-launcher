use tauri::{AppHandle, State};

use crate::models::game_client::GameClientSnapshot;
use crate::models::launch::LaunchValues;
use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::launcher;

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, GameState>,
    client_id: String,
    server: ServerConfig,
    runner: Option<String>,
    launch_values: Option<LaunchValues>,
) -> Result<GameClientSnapshot, String> {
    server.validate_executable_available()?;
    let tool_lifecycle = state.tool_lifecycle.lock().await;
    let had_clients = state.game.active_count()? > 0;
    let reservation = state
        .game
        .begin_launch(client_id, server.id.clone(), server.name.clone())?;
    if had_clients {
        if let Err(error) = launcher::stop_tools_for_additional_client(&state).await {
            state.game.cancel_launch(reservation);
            return Err(format!(
                "No se pudieron detener las herramientas antes de abrir otro cliente: {error}"
            ));
        }
    }
    drop(tool_lifecycle);
    let result = launcher::launch_game(
        app,
        state.game.clone(),
        reservation,
        launcher::LaunchTools {
            autopot: &state.autopot,
            autobuff: &state.autobuff,
            spammer: &state.spammer,
            input: &state.input,
        },
        server,
        runner,
        launch_values.unwrap_or_default(),
    )
    .await;
    if result.is_err() {
        state.game.cancel_launch(reservation);
    }
    result
}

#[tauri::command]
pub async fn stop_game(state: State<'_, GameState>, client_id: String) -> Result<(), String> {
    let _tool_lifecycle = state.tool_lifecycle.lock().await;
    launcher::stop_game(&state, &client_id).await
}

#[tauri::command]
pub async fn stop_all_games(state: State<'_, GameState>) -> Result<(), String> {
    let _tool_lifecycle = state.tool_lifecycle.lock().await;
    launcher::stop_all_games(&state).await
}

#[tauri::command]
pub fn list_game_clients(state: State<'_, GameState>) -> Result<Vec<GameClientSnapshot>, String> {
    state.game.snapshots()
}
