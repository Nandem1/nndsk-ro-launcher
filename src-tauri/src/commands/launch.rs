use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use std::sync::Arc;

use crate::commands::audio::{self, mmdevapi_recovery_hint};
use crate::commands::check::get_prefix_path;
use crate::commands::runner::resolve_runner;
use crate::commands::settings::load_settings;
use crate::commands::setup::ensure_gecko;
use crate::models::server::ServerConfig;
use crate::GameState;

#[derive(Serialize, Clone)]
struct LogEvent {
    line: String,
}

#[derive(Serialize, Clone)]
struct ExitEvent {
    code: i32,
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: tauri::State<'_, GameState>,
    server: ServerConfig,
) -> Result<(), String> {
    let prefix_path = server
        .wine_prefix
        .clone()
        .unwrap_or_else(get_prefix_path);

    let marker = format!("{}/.ro-launcher-configured", prefix_path);
    if !std::path::Path::new(&marker).exists() {
        return Err(
            "El WINEPREFIX no está configurado. Ejecuta el setup primero.".to_string(),
        );
    }

    let runner_path = match server.runner.clone() {
        Some(path) if !path.is_empty() => path,
        _ => load_settings().await?.default_runner,
    };
    let resolved = resolve_runner(&runner_path)?;

    ensure_gecko(&app, &prefix_path).await?;
    audio::ensure_audio_driver(Some(&app), &prefix_path, &resolved).await?;

    let exe_path = server.executable_path.clone();
    let work_dir = std::path::Path::new(&exe_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut cmd = Command::new(&resolved.wine_bin);
    cmd.arg(&exe_path)
        .env("WINEPREFIX", &prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .env("DXVK_ASYNC", "1")
        .env("DXVK_CONFIG", "d3d9.forceSamplerTypeSpecConstants=True")
        .env("WINE_LARGE_ADDRESS_AWARE", "1")
        .env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b")
        .current_dir(&work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(proton_libs) = &resolved.ld_library_path {
        let ld_library_path = match std::env::var("LD_LIBRARY_PATH") {
            Ok(existing) if !existing.is_empty() => format!("{proton_libs}:{existing}"),
            _ => proton_libs.clone(),
        };
        cmd.env("LD_LIBRARY_PATH", ld_library_path);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al lanzar el juego: {}", e))?;

    if let Some(pid) = child.id() {
        *state.pid.lock().unwrap() = Some(pid);
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let app1 = app.clone();
    let h1 = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut lines = tokio::io::BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.contains("fixme:") {
                    let _ = app1.emit("ro-launcher://log", LogEvent { line });
                }
            }
        }
    });

    let app2 = app.clone();
    let h2 = tokio::spawn(async move {
        if let Some(err) = stderr {
            let mut lines = tokio::io::BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if audio::is_mmdevapi_audio_error(&line) {
                    let _ = app2.emit("ro-launcher://log", LogEvent {
                        line: mmdevapi_recovery_hint().to_string(),
                    });
                }
                if line.contains("err:") || (!line.contains("fixme:") && !line.is_empty()) {
                    let _ = app2.emit("ro-launcher://log", LogEvent { line });
                }
            }
        }
    });

    let pid_state = Arc::clone(&state.pid);
    tokio::spawn(async move {
        let _ = tokio::join!(h1, h2);
        let code = child
            .wait()
            .await
            .map(|s| s.code().unwrap_or(-1))
            .unwrap_or(-1);
        *pid_state.lock().unwrap() = None;
        let _ = app.emit("ro-launcher://game-exit", ExitEvent { code });
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_game(state: tauri::State<'_, GameState>) -> Result<(), String> {
    let pid = state.pid.lock().unwrap().take();
    if let Some(pid) = pid {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
    }
    Ok(())
}
