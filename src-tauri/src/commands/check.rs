use serde::Serialize;
use std::path::Path;

use crate::commands::audio::{self, AudioDriver};
use crate::commands::runner::resolve_runner;
use crate::commands::settings::load_settings;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub wine: bool,
    pub winetricks: bool,
    pub dxvk: bool,
    pub prefix_configured: bool,
    pub audio_ok: bool,
    pub audio_driver: String,
    pub audio_warning: Option<String>,
}

#[tauri::command]
pub async fn check_dependencies(runner: Option<String>) -> Result<DependencyStatus, String> {
    let wine = Path::new("/usr/bin/wine-cachyos").exists() || Path::new("/usr/bin/wine").exists();
    let winetricks = Path::new("/usr/bin/winetricks").exists();

    let prefix_path = get_prefix_path();
    let dxvk = check_dxvk_installed(&prefix_path);
    let prefix_configured = check_prefix_configured(&prefix_path);

    let runner_path = match runner {
        Some(path) if !path.is_empty() => path,
        _ => load_settings().await?.default_runner,
    };
    let resolved = resolve_runner(&runner_path)?;

    let current_driver = if prefix_configured {
        audio::read_current_driver(&prefix_path, &resolved).await
    } else {
        None
    };

    let audio_status = audio::detect_audio_backends(current_driver);
    let audio_driver = audio_status
        .current_driver
        .or(Some(audio_status.recommended))
        .unwrap_or(AudioDriver::None);

    Ok(DependencyStatus {
        wine,
        winetricks,
        dxvk,
        prefix_configured,
        audio_ok: audio_status.ok,
        audio_driver: audio_driver.as_str().to_string(),
        audio_warning: audio_status.warning,
    })
}

pub fn get_prefix_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{}/.local/share/ro-launcher/prefix", home)
}

pub fn check_gecko_installed(prefix_path: &str) -> bool {
    gecko_has_runtime(&format!(
        "{prefix_path}/drive_c/windows/system32/gecko"
    )) || gecko_has_runtime(&format!(
        "{prefix_path}/drive_c/windows/syswow64/gecko"
    ))
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

fn check_dxvk_installed(prefix_path: &str) -> bool {
    Path::new(&format!(
        "{}/drive_c/windows/system32/d3d9.dll",
        prefix_path
    ))
    .exists()
}

fn check_prefix_configured(prefix_path: &str) -> bool {
    Path::new(&format!("{}/.ro-launcher-configured", prefix_path)).exists()
}
