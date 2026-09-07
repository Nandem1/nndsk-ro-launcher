use tauri::{AppHandle, State};

use crate::models::autopot::AutopotStatusEvent;
use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::autopot::{
    load_profiles, start_session, DetectedNameAddress, LevelScanProgress, MapScanProgress,
    MemoryScanProgress,
};
use crate::tools::presence::parse_address_override;
use crate::utils::emit_tool_log_opt;

#[tauri::command]
pub async fn start_autopot(
    app: AppHandle,
    state: State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    let _tool_lifecycle = state.tool_lifecycle.lock().await;
    server.validate_executable_available()?;
    let launcher_pid = state.game.sole_running_pid_for(&server.id)?;

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
    let _tool_lifecycle = state.tool_lifecycle.lock().await;
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
    let pid = state.game.sole_running_pid()?;
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
    hp_base: Option<String>,
) -> Result<DetectedNameAddress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de buscar la dirección del nombre".into());
    }
    let pid = state.game.sole_running_pid()?;
    let result = state
        .autopot
        .find_name_address(
            pid,
            character_name,
            parse_address_override(hp_base.as_deref()),
        )
        .await?;
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Nombre '{}' encontrado en {} (cerca de HP)",
            result.character_name, result.name_address
        ),
    );
    Ok(result)
}

#[tauri::command]
pub async fn begin_autopot_level_scan(
    app: AppHandle,
    state: State<'_, GameState>,
    current_level: u32,
    name_address: Option<String>,
    hp_base: Option<String>,
) -> Result<LevelScanProgress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de buscar una dirección de memoria".into());
    }
    let pid = state.game.sole_running_pid()?;
    emit_tool_log_opt(
        Some(&app),
        format!("[Presence] Escaneo de nivel PID={pid} nv={current_level}"),
    );
    let result = state
        .autopot
        .begin_level_scan(
            pid,
            current_level,
            parse_address_override(name_address.as_deref()),
            parse_address_override(hp_base.as_deref()),
        )
        .await?;
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[Presence] Escaneo de nivel: {} candidatos",
            result.candidate_count
        ),
    );
    Ok(result)
}

#[tauri::command]
pub async fn refine_autopot_level_scan(
    app: AppHandle,
    state: State<'_, GameState>,
    current_level: u32,
) -> Result<LevelScanProgress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de continuar el escaneo de memoria".into());
    }
    let result = state.autopot.refine_level_scan(current_level).await?;
    if let Some(found) = &result.confirmed {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Presence] Nivel confirmado {} (job={})",
                found.level_address,
                found.job_level_address.as_deref().unwrap_or("—")
            ),
        );
    } else {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Presence] Refinado nivel={current_level}: {} candidatos",
                result.candidate_count
            ),
        );
    }
    Ok(result)
}

#[tauri::command]
pub async fn begin_autopot_map_scan(
    app: AppHandle,
    state: State<'_, GameState>,
    map_name: String,
    name_address: Option<String>,
    hp_base: Option<String>,
    level_address: Option<String>,
) -> Result<MapScanProgress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de buscar una dirección de memoria".into());
    }
    let pid = state.game.sole_running_pid()?;
    emit_tool_log_opt(
        Some(&app),
        format!("[Presence] Escaneo de mapa PID={pid} map={map_name}"),
    );
    let result = state
        .autopot
        .begin_map_scan(
            pid,
            map_name,
            parse_address_override(name_address.as_deref()),
            parse_address_override(hp_base.as_deref()),
            parse_address_override(level_address.as_deref()),
        )
        .await?;
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[Presence] Escaneo de mapa: {} candidatos",
            result.candidate_count
        ),
    );
    Ok(result)
}

#[tauri::command]
pub async fn refine_autopot_map_scan(
    app: AppHandle,
    state: State<'_, GameState>,
    map_name: String,
) -> Result<MapScanProgress, String> {
    if state.autopot.status().active {
        return Err("Detén AutoPot antes de continuar el escaneo de memoria".into());
    }
    let result = state.autopot.refine_map_scan(map_name).await?;
    if let Some(found) = &result.confirmed {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Presence] Mapa '{}' confirmado {}",
                found.map_name, found.map_address
            ),
        );
    } else {
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Presence] Refinado mapa: {} candidatos",
                result.candidate_count
            ),
        );
    }
    Ok(result)
}
