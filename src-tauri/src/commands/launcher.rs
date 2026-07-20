use tauri::{AppHandle, State};

use crate::models::launch::LaunchValues;
use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::launcher;

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
    runner: Option<String>,
    launch_values: Option<LaunchValues>,
) -> Result<(), String> {
    server.validate_executable_available()?;
    let reservation = state.game.begin_launch()?;
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
pub async fn stop_game(state: State<'_, GameState>) -> Result<(), String> {
    launcher::stop_game(&state).await
}
