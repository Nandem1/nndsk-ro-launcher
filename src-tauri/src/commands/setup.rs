use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use crate::commands::audio;
use crate::commands::check::get_prefix_path;
use crate::commands::gecko::install_gecko;
use crate::commands::runner::resolve_runner;
use crate::commands::settings::load_settings;

#[derive(Serialize, Clone)]
struct ProgressEvent {
    step: String,
    percent: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct LogEvent {
    line: String,
}

#[tauri::command]
pub async fn setup_prefix(app: AppHandle) -> Result<(), String> {
    let prefix_path = get_prefix_path();

    emit_progress(&app, "Creando WINEPREFIX...", 5)?;
    std::fs::create_dir_all(&prefix_path).map_err(|e| e.to_string())?;

    let _ = tokio::process::Command::new("wineserver")
        .arg("-k")
        .env("WINEPREFIX", &prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .status()
        .await;

    emit_progress(&app, "Inicializando WINEPREFIX...", 10)?;
    run_cmd(&app, &prefix_path, "wineboot", &["-i"]).await?;

    emit_progress(&app, "Instalando Wine Gecko...", 20)?;
    install_gecko(&app, &prefix_path, "/usr/bin/wine").await?;

    emit_progress(&app, "Instalando DXVK...", 35)?;
    run_winetricks(&app, &prefix_path, &["dxvk"]).await?;

    emit_progress(&app, "Instalando vcredist_2019...", 55)?;
    run_winetricks(&app, &prefix_path, &["vcrun2019"]).await?;

    emit_progress(&app, "Instalando d3dx9...", 75)?;
    run_winetricks(&app, &prefix_path, &["d3dx9"]).await?;

    emit_progress(&app, "Instalando corefonts...", 90)?;
    run_winetricks(&app, &prefix_path, &["corefonts"]).await?;

    emit_progress(&app, "Configurando audio...", 95)?;
    let runner_path = load_settings().await?.default_runner;
    if let Ok(resolved) = resolve_runner(&runner_path) {
        let _ = audio::ensure_audio_driver(Some(&app), &prefix_path, &resolved).await;
    }

    std::fs::write(
        format!("{}/.ro-launcher-configured", prefix_path),
        "configured",
    )
    .map_err(|e| e.to_string())?;

    emit_progress(&app, "¡Listo!", 100)?;
    Ok(())
}

#[tauri::command]
pub async fn reset_prefix(app: AppHandle) -> Result<(), String> {
    let prefix_path = get_prefix_path();

    emit_progress(&app, "Deteniendo Wine...", 0)?;
    let _ = Command::new("wineserver")
        .arg("-k")
        .env("WINEPREFIX", &prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .status()
        .await;

    if Path::new(&prefix_path).exists() {
        let _ = app.emit(
            "ro-launcher://log",
            LogEvent {
                line: format!("Eliminando prefix en {prefix_path}..."),
            },
        );
        std::fs::remove_dir_all(&prefix_path)
            .map_err(|e| format!("Error al eliminar el prefix: {e}"))?;
    }

    setup_prefix(app).await
}

pub async fn ensure_gecko(app: &AppHandle, prefix_path: &str) -> Result<(), String> {
    install_gecko(app, prefix_path, "/usr/bin/wine").await
}

fn emit_progress(app: &AppHandle, step: &str, percent: u32) -> Result<(), String> {
    app.emit(
        "ro-launcher://progress",
        ProgressEvent {
            step: step.to_string(),
            percent,
        },
    )
    .map_err(|e| e.to_string())
}

async fn run_winetricks(
    app: &AppHandle,
    prefix_path: &str,
    packages: &[&str],
) -> Result<(), String> {
    let mut cmd = Command::new("winetricks");
    cmd.arg("-q");
    for pkg in packages {
        cmd.arg(pkg);
    }
    cmd.env("WINEPREFIX", prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al ejecutar winetricks: {}", e))?;

    drain_and_log(app, &mut child).await;

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!(
            "winetricks falló con código: {:?}",
            status.code()
        ));
    }
    Ok(())
}

async fn run_cmd(
    app: &AppHandle,
    prefix_path: &str,
    prog: &str,
    args: &[&str],
) -> Result<(), String> {
    let mut cmd = Command::new(prog);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.env("WINEPREFIX", prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al ejecutar {}: {}", prog, e))?;

    drain_and_log(app, &mut child).await;
    child.wait().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn drain_and_log(app: &AppHandle, child: &mut tokio::process::Child) {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let app1 = app.clone();
    let h1 = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut lines = tokio::io::BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.contains("fixme:") && !line.contains("libEGL warning") {
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
                if !line.contains("fixme:") && !line.contains("libEGL warning") {
                    let _ = app2.emit("ro-launcher://log", LogEvent { line });
                }
            }
        }
    });

    let _ = tokio::join!(h1, h2);
}
