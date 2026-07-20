use tauri::{AppHandle, State};

use crate::models::autopot::AutopotStatusEvent;
use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::autopot::{
    load_profiles, start_session, DetectedNameAddress, MemoryScanProgress,
};
use crate::utils::emit_tool_log_opt;

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

#[tauri::command]
pub async fn begin_autopot_memory_scan(
    app: AppHandle,
    state: State<'_, GameState>,
    current_hp: u32,
) -> Result<MemoryScanProgress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de buscar una dirección de memoria".into());
    }
    let pid = state
        .game
        .running_pid()?
        .ok_or_else(|| "Inicia el juego antes de buscar la dirección de HP".to_string())?;
    emit_tool_log_opt(
        Some(&app),
        format!("[AutoPot] Escaneo inicial PID={pid} HP={current_hp}"),
    );
    let result = state.autopot.begin_memory_scan(pid, current_hp).await?;
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Escaneo inicial: {} candidatos",
            result.candidate_count
        ),
    );
    Ok(result)
}

#[tauri::command]
pub async fn refine_autopot_memory_scan(
    app: AppHandle,
    state: State<'_, GameState>,
    current_hp: u32,
) -> Result<MemoryScanProgress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de continuar el escaneo de memoria".into());
    }
    let result = state.autopot.refine_memory_scan(current_hp).await?;
    if let Some(layout) = &result.confirmed {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[AutoPot] Dirección confirmada {} | HP={}/{} SP={}/{} status={}",
                layout.hp_base,
                layout.current_hp,
                layout.max_hp,
                layout.current_sp,
                layout.max_sp,
                layout.status_buffer,
            ),
        );
    } else {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[AutoPot] Refinado HP={current_hp}: {} candidatos",
                result.candidate_count
            ),
        );
    }
    Ok(result)
}

#[tauri::command]
pub fn cancel_autopot_memory_scan(state: State<'_, GameState>) {
    state.autopot.cancel_memory_scan();
}

#[tauri::command]
pub async fn find_autopot_name_address(
    app: AppHandle,
    state: State<'_, GameState>,
    character_name: String,
) -> Result<DetectedNameAddress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de buscar la dirección del nombre".into());
    }
    let pid = state
        .game
        .running_pid()?
        .ok_or_else(|| "Inicia el juego antes de buscar el nombre".to_string())?;
    let result = state.autopot.find_name_address(pid, character_name).await?;
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Nombre '{}' encontrado en {} (primera coincidencia)",
            result.character_name, result.name_address
        ),
    );
    Ok(result)
}
