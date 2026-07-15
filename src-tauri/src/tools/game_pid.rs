use std::time::Duration;

use ro_tools_core::ClientProfile;
use ro_tools_linux::resolve_best_game_pid;
use tauri::AppHandle;
use tokio::time::sleep;

use crate::utils::emit_tool_log_opt;

const PID_RESOLVE_ATTEMPTS: u32 = 20;
const PID_RESOLVE_DELAY: Duration = Duration::from_millis(500);

pub async fn resolve_game_pid_with_retry(
    app: &AppHandle,
    tool_name: &str,
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
            format!(
                "[{tool_name}] PID no encontrado (intento {attempt}/{PID_RESOLVE_ATTEMPTS})..."
            ),
        );
        sleep(PID_RESOLVE_DELAY).await;
    }

    Err("No se pudo resolver el PID del cliente RO. ¿Está abierto y logueado?".into())
}
