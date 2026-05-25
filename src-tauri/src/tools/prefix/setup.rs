use std::path::Path;

use tauri::AppHandle;
use tokio::process::Command;

use crate::utils::audio;
use crate::utils::gecko::install_gecko;
use crate::utils::process::run_logged_command_ok;
use crate::utils::resolve_effective_runner;
use crate::utils::{
    apply_prefix_env, emit_log, emit_progress, kill_wineserver, prefix_path, write_prefix_marker,
    WINETRICKS_BIN,
};

pub async fn setup_prefix(app: AppHandle) -> Result<(), String> {
    let prefix = prefix_path();
    let resolved = resolve_effective_runner(None).await?;

    emit_progress(&app, "Creando WINEPREFIX...", 5)?;
    std::fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;

    kill_wineserver(&prefix).await;

    emit_progress(&app, "Inicializando WINEPREFIX...", 10)?;
    run_cmd(&app, &prefix, "wineboot", &["-i"]).await?;

    emit_progress(&app, "Instalando Wine Gecko...", 20)?;
    install_gecko(&app, &prefix, &resolved.wine_bin).await?;

    emit_progress(&app, "Instalando DXVK...", 35)?;
    run_winetricks(&app, &prefix, &["dxvk"]).await?;

    emit_progress(&app, "Instalando vcredist_2019...", 55)?;
    run_winetricks(&app, &prefix, &["vcrun2019"]).await?;

    emit_progress(&app, "Instalando d3dx9...", 75)?;
    run_winetricks(&app, &prefix, &["d3dx9"]).await?;

    emit_progress(&app, "Instalando corefonts...", 90)?;
    run_winetricks(&app, &prefix, &["corefonts"]).await?;

    emit_progress(&app, "Configurando audio...", 95)?;
    let _ = audio::ensure_audio_driver(Some(&app), &prefix, &resolved).await;

    write_prefix_marker(&prefix)?;

    emit_progress(&app, "¡Listo!", 100)?;
    Ok(())
}

pub async fn reset_prefix(app: AppHandle) -> Result<(), String> {
    let prefix = prefix_path();

    emit_progress(&app, "Deteniendo Wine...", 0)?;
    kill_wineserver(&prefix).await;

    if Path::new(&prefix).exists() {
        emit_log(
            &app,
            format!("Eliminando prefix en {prefix}..."),
        )?;
        std::fs::remove_dir_all(&prefix)
            .map_err(|e| format!("Error al eliminar el prefix: {e}"))?;
    }

    setup_prefix(app).await
}

async fn run_winetricks(
    app: &AppHandle,
    prefix_path: &str,
    packages: &[&str],
) -> Result<(), String> {
    let mut cmd = Command::new(WINETRICKS_BIN);
    cmd.arg("-q");
    for pkg in packages {
        cmd.arg(pkg);
    }
    apply_prefix_env(&mut cmd, prefix_path);
    run_logged_command_ok(app, cmd, "winetricks").await
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
    apply_prefix_env(&mut cmd, prefix_path);
    run_logged_command_ok(app, cmd, prog).await
}
