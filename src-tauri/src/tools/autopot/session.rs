use std::sync::Arc;
use std::time::Duration;

use ro_tools_core::ClientProfile;
use ro_tools_linux::resolve_best_game_pid;
use tauri::AppHandle;
use tokio::time::sleep;

use crate::models::server::ServerConfig;
use crate::tools::autopot::{load_profiles, resolve_profile, AutopotHandle};
use crate::tools::input::{ensure_ydotoold, YdotoolDaemon};
use crate::utils::{effective_prefix, emit_tool_log_opt};

const PID_RESOLVE_ATTEMPTS: u32 = 20;
const PID_RESOLVE_DELAY_MS: u64 = 500;

/// Orquesta arranque de AutoPot: resuelve PID, valida input y delega al servicio.
pub async fn start_session(
    app: AppHandle,
    handle: &AutopotHandle,
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
        .start(
            app,
            pid,
            server.autopot.clone(),
            profile,
            ydotoold,
        )
        .await
}

async fn resolve_game_pid_with_retry(
    app: &AppHandle,
    launcher_pid: u32,
    exe_path: &str,
    prefix: &str,
    profile: &ClientProfile,
) -> Result<(u32, String), String> {
    for attempt in 1..=PID_RESOLVE_ATTEMPTS {
        if let Some(found) = resolve_best_game_pid(launcher_pid, exe_path, prefix, profile) {
            return Ok(found);
        }
        emit_tool_log_opt(
            Some(app),
            format!("[AutoPot] PID no encontrado (intento {attempt}/{PID_RESOLVE_ATTEMPTS})..."),
        );
        sleep(Duration::from_millis(PID_RESOLVE_DELAY_MS)).await;
    }

    Err("No se pudo resolver el PID del cliente RO. ¿Está abierto y logueado?".into())
}
