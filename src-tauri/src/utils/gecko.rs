use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tokio::process::Command;

use crate::utils::process::run_logged_command_ok;
use crate::utils::{app_data_dir, emit_log, ResolvedRunner};

const GECKO_VERSION: &str = "2.47.4";
const GECKO_BASE_URL: &str = "https://dl.winehq.org/wine/wine-gecko";

pub fn find_system_gecko_msis() -> Vec<PathBuf> {
    let mut msis = Vec::new();
    let search_dirs = ["/usr/share/wine/gecko", "/usr/share/wine/wine/gecko"];

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

pub fn check_gecko_installed(prefix_path: &str) -> bool {
    gecko_has_runtime(&format!("{prefix_path}/drive_c/windows/system32/gecko"))
        || gecko_has_runtime(&format!("{prefix_path}/drive_c/windows/syswow64/gecko"))
}

fn gecko_has_runtime(gecko_dir: &str) -> bool {
    dir_contains_xul(Path::new(gecko_dir))
}

fn dir_contains_xul(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    if dir.join("xul.dll").is_file() {
        return true;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && dir_contains_xul(&path) {
            return true;
        }
    }

    false
}

fn gecko_cache_dir() -> PathBuf {
    app_data_dir().join("cache/gecko")
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
            emit_log(app, format!("Descargando Wine Gecko ({file})..."))?;
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
        return Err(
            "No se pudo descargar Wine Gecko. Verifica tu conexión a internet e intenta de nuevo."
                .to_string(),
        );
    }

    Ok(())
}

pub async fn install_gecko_for_runner(
    app: &AppHandle,
    prefix_path: &str,
    runner: &ResolvedRunner,
) -> Result<(), String> {
    // Proton sincroniza Gecko desde su default_pfx. Instalar los MSI de Wine encima puede
    // reemplazar archivos administrados por Proton y romper upgrades posteriores.
    if runner.is_proton() {
        return Ok(());
    }

    if check_gecko_installed(prefix_path) {
        return Ok(());
    }

    let msis = ensure_cached_gecko_msis(app).await?;

    emit_log(app, "Instalando Wine Gecko en el prefix...")?;

    for msi in msis {
        let msi_str = msi.to_string_lossy();
        let cmd = runner.builtin_command(prefix_path, "msiexec", ["/i", msi_str.as_ref(), "/qn"]);
        run_logged_command_ok(app, cmd, &format!("wine msiexec {msi_str}")).await?;
    }

    if !check_gecko_installed(prefix_path) {
        return Err(
            "Wine Gecko no quedó instalado en el prefix. Intenta rearmar el WINEPREFIX."
                .to_string(),
        );
    }

    Ok(())
}
