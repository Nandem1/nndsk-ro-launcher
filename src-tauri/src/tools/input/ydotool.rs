use ro_tools_linux::{
    autopot_input_installed, is_ydotool_responsive, is_ydotool_socket_ready,
    remove_stale_ydotool_socket, ydotool_socket_path,
};
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::sleep;

use crate::utils::emit_tool_log_opt;
use tauri::AppHandle;

const YDOTOOL_INSTALL_HINT: &str = "Instálalo con: sudo pacman -S ydotool";

pub struct YdotoolDaemon {
    child: Mutex<Option<tokio::process::Child>>,
}

impl YdotoolDaemon {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    pub async fn shutdown(&self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill().await;
        }
    }
}

/// Campos de input virtual para [`DependencyStatus`] (solo paquetes; no exige daemon activo).
pub fn dependency_autopot_input_fields() -> (bool, Option<String>) {
    if autopot_input_installed() {
        return (true, None);
    }

    (
        false,
        Some(format!("Opcional para AutoPot. {YDOTOOL_INSTALL_HINT}")),
    )
}

/// Garantiza ydotoold activo antes de potear. Arranca el daemon si hace falta.
pub async fn ensure_ydotoold(
    app: Option<&AppHandle>,
    daemon: &YdotoolDaemon,
) -> Result<(), String> {
    if !autopot_input_installed() {
        return Err(format!("ydotool no está instalado. {YDOTOOL_INSTALL_HINT}"));
    }

    for _ in 0..6 {
        if is_ydotool_responsive() {
            emit_tool_log_opt(app, "[Input] ydotoold activo");
            return Ok(());
        }
        if is_ydotool_socket_ready() {
            emit_tool_log_opt(app, "[Input] Socket ydotool obsoleto, reiniciando...");
            remove_stale_ydotool_socket();
        }
        sleep(Duration::from_millis(200)).await;
    }

    let stale_child = daemon.child.lock().unwrap().take();
    if let Some(mut child) = stale_child {
        let _ = child.kill().await;
    }
    remove_stale_ydotool_socket();

    let socket_path = ydotool_socket_path();
    let uid = ro_tools_linux::current_uid();
    let gid = ro_tools_linux::current_gid();

    emit_tool_log_opt(
        app,
        format!("[Input] Iniciando ydotoold ({socket_path})"),
    );

    let mut child = Command::new("ydotoold")
        .arg("--socket-path")
        .arg(&socket_path)
        .arg("--socket-own")
        .arg(format!("{uid}:{gid}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("No se pudo iniciar ydotoold: {e}"))?;

    for _ in 0..30 {
        if is_ydotool_socket_ready() {
            *daemon.child.lock().unwrap() = Some(child);
            emit_tool_log_opt(app, "[Input] ydotoold listo");
            return Ok(());
        }

        if let Ok(Some(status)) = child.try_wait() {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr).await;
            }
            let detail = stderr.trim();
            let msg = if detail.is_empty() {
                format!("ydotoold terminó ({status})")
            } else {
                format!("ydotoold terminó ({status}): {detail}")
            };
            return Err(permission_hint(&msg));
        }

        sleep(Duration::from_millis(100)).await;
    }

    let _ = child.kill().await;
    Err(permission_hint(
        "ydotoold no respondió a tiempo (¿permisos uinput?)",
    ))
}

fn permission_hint(base: &str) -> String {
    format!(
        "{base}. Si persiste: sudo usermod -aG input $USER y reinicia sesión."
    )
}
