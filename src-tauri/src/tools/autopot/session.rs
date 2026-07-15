use std::sync::Arc;
use tauri::AppHandle;

use crate::models::server::ServerConfig;
use crate::tools::autopot::{load_profiles, resolve_profile, AutopotHandle};
use crate::tools::game_pid::resolve_game_pid_with_retry;
use crate::tools::input::{ensure_ydotoold, InputGateway, YdotoolDaemon};
use crate::utils::{effective_prefix, emit_tool_log_opt};

/// Orquesta arranque de AutoPot: resuelve PID, valida input y delega al servicio.
pub async fn start_session(
    app: AppHandle,
    handle: &AutopotHandle,
    input: InputGateway,
    ydotoold: Arc<YdotoolDaemon>,
    launcher_pid: u32,
    server: ServerConfig,
) -> Result<(), String> {
    let prefix = effective_prefix(server.wine_prefix.clone());
    let profiles = load_profiles();
    let profile = resolve_profile(&profiles, &server.executable_path, &server.autopot);

    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Buscando PID | launcher={launcher_pid} exe={} prefix={prefix}",
            server.executable_path
        ),
    );
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Perfil '{}' HP={:#x} name={:#x}",
            profile.label, profile.hp_base, profile.name_address
        ),
    );

    let (pid, detail) = resolve_game_pid_with_retry(
        &app,
        "AutoPot",
        launcher_pid,
        &server.executable_path,
        &prefix,
        &profile,
    )
    .await?;

    emit_tool_log_opt(
        Some(&app),
        format!("[AutoPot] PID seleccionado: {pid} ({detail})"),
    );

    ensure_ydotoold(Some(&app), ydotoold.as_ref()).await?;

    handle
        .start(app, pid, server.autopot.clone(), profile, input, ydotoold)
        .await
}
