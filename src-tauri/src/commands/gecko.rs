use serde::Serialize;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use crate::commands::check::check_gecko_installed;

#[derive(Clone, Serialize)]
struct LogEvent {
    line: String,
}

const GECKO_VERSION: &str = "2.47.4";
const GECKO_BASE_URL: &str = "https://dl.winehq.org/wine/wine-gecko";

pub fn find_system_gecko_msis() -> Vec<PathBuf> {
    let mut msis = Vec::new();
    let search_dirs = [
        "/usr/share/wine/gecko",
        "/usr/share/wine/wine/gecko",
    ];

    for dir in search_dirs {
        collect_msis(Path::new(dir), &mut msis);
    }

    msis.sort();
    msis.dedup();
    msis
}

fn collect_msis(dir: &Path, msis: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("msi") {
            msis.push(path);
        }
    }
}

fn gecko_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!("{home}/.local/share/ro-launcher/cache/gecko"))
}

async fn ensure_cached_gecko_msis(app: &AppHandle) -> Result<Vec<PathBuf>, String> {
    let system_msis = find_system_gecko_msis();
    if !system_msis.is_empty() {
        return Ok(system_msis);
    }

    let cache_dir = gecko_cache_dir();
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let files = [
        format!("wine-gecko-{GECKO_VERSION}-x86_64.msi"),
        format!("wine-gecko-{GECKO_VERSION}-x86.msi"),
    ];

    let mut msis = Vec::new();
    for file in files {
        let dest = cache_dir.join(&file);
        if !dest.exists() {
            let url = format!("{GECKO_BASE_URL}/{GECKO_VERSION}/{file}");
            let _ = app.emit(
                "ro-launcher://log",
                LogEvent {
                    line: format!("Descargando Wine Gecko ({file})..."),
                },
            );
            download_file(&url, &dest).await?;
        }
        msis.push(dest);
    }

    Ok(msis)
}

async fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let dest_str = dest.to_string_lossy();
    let status = Command::new("curl")
        .args(["-fsSL", "-o", dest_str.as_ref(), url])
        .status()
        .await
        .map_err(|e| format!("Error al descargar Gecko: {e}"))?;

    if !status.success() {
        return Err(format!(
            "No se pudo descargar Wine Gecko. Verifica tu conexión a internet e intenta de nuevo."
        ));
    }

    Ok(())
}

pub async fn install_gecko(
    app: &AppHandle,
    prefix_path: &str,
    wine_bin: &str,
) -> Result<(), String> {
    if check_gecko_installed(prefix_path) {
        return Ok(());
    }

    let msis = ensure_cached_gecko_msis(app).await?;

    let _ = app.emit(
        "ro-launcher://log",
        LogEvent {
            line: "Instalando Wine Gecko en el prefix...".to_string(),
        },
    );

    for msi in msis {
        let msi_str = msi.to_string_lossy();
        run_wine(
            app,
            prefix_path,
            wine_bin,
            &["msiexec", "/i", msi_str.as_ref(), "/qn"],
        )
        .await?;
    }

    if !check_gecko_installed(prefix_path) {
        return Err(
            "Wine Gecko no quedó instalado en el prefix. Intenta rearmar el WINEPREFIX.".to_string(),
        );
    }

    Ok(())
}

async fn run_wine(
    app: &AppHandle,
    prefix_path: &str,
    wine_bin: &str,
    args: &[&str],
) -> Result<(), String> {
    let mut cmd = Command::new(wine_bin);
    cmd.args(args)
        .env("WINEPREFIX", prefix_path)
        .env("WAYLAND_DISPLAY", "")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al ejecutar {wine_bin}: {e}"))?;

    drain_and_log(app, &mut child).await;

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!(
            "Comando falló ({wine_bin} {}): {:?}",
            args.join(" "),
            status.code()
        ));
    }

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
                if should_log_line(&line) {
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
                if should_log_line(&line) {
                    let _ = app2.emit("ro-launcher://log", LogEvent { line });
                }
            }
        }
    });

    let _ = tokio::join!(h1, h2);
}

fn should_log_line(line: &str) -> bool {
    !line.contains("fixme:") && !line.contains("libEGL warning")
}
