use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use crate::commands::runner::ResolvedRunner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioDriver {
    Pulse,
    Alsa,
    None,
}

impl AudioDriver {
    pub fn as_reg_value(self) -> Option<&'static str> {
        match self {
            AudioDriver::Pulse => Some("pulse"),
            AudioDriver::Alsa => Some("alsa"),
            AudioDriver::None => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AudioDriver::Pulse => "PulseAudio",
            AudioDriver::Alsa => "ALSA",
            AudioDriver::None => "ninguno",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AudioDriver::Pulse => "pulse",
            AudioDriver::Alsa => "alsa",
            AudioDriver::None => "none",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioBackendStatus {
    pub pulse_32: bool,
    pub alsa_32: bool,
    pub current_driver: Option<AudioDriver>,
    pub recommended: AudioDriver,
    pub ok: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureAudioResult {
    pub configured: bool,
    pub driver: AudioDriver,
    pub message: Option<String>,
}

#[derive(Clone, Serialize)]
struct LogEvent {
    line: String,
}

pub fn lib32_pulse_available() -> bool {
    Path::new("/usr/lib32/libpulse.so.0").exists()
}

pub fn lib32_alsa_available() -> bool {
    Path::new("/usr/lib32/libasound.so.2").exists()
}

pub fn recommended_driver() -> AudioDriver {
    if lib32_pulse_available() {
        AudioDriver::Pulse
    } else if lib32_alsa_available() {
        AudioDriver::Alsa
    } else {
        AudioDriver::None
    }
}

pub fn detect_audio_backends(current_driver: Option<AudioDriver>) -> AudioBackendStatus {
    let pulse_32 = lib32_pulse_available();
    let alsa_32 = lib32_alsa_available();
    let recommended = recommended_driver();
    let ok = pulse_32 || alsa_32;

    let warning = if !pulse_32 && alsa_32 {
        Some(
            "Audio en ALSA (fallback). Para PulseAudio: sudo pacman -S lib32-libpulse"
                .to_string(),
        )
    } else if !ok {
        Some(
            "Sin librerías de audio 32-bit. Instala lib32-libpulse o lib32-alsa-lib."
                .to_string(),
        )
    } else {
        None
    };

    AudioBackendStatus {
        pulse_32,
        alsa_32,
        current_driver,
        recommended,
        ok,
        warning,
    }
}

pub fn is_mmdevapi_audio_error(line: &str) -> bool {
    line.contains("err:mmdevapi")
        && (line.contains("load_driver") || line.contains("DllGetClassObject"))
}

pub fn mmdevapi_recovery_hint() -> &'static str {
    "Fallo de audio detectado. El launcher intentará usar ALSA en el próximo lanzamiento. \
     Si persiste: sudo pacman -S lib32-libpulse lib32-alsa-lib"
}

fn apply_runner_env(cmd: &mut Command, runner: &ResolvedRunner) {
    if let Some(proton_libs) = &runner.ld_library_path {
        let ld_library_path = match std::env::var("LD_LIBRARY_PATH") {
            Ok(existing) if !existing.is_empty() => format!("{proton_libs}:{existing}"),
            _ => proton_libs.clone(),
        };
        cmd.env("LD_LIBRARY_PATH", ld_library_path);
    }
}

fn parse_driver_from_reg_output(output: &str) -> Option<AudioDriver> {
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Audio") {
            continue;
        }
        let value = trimmed.split("REG_SZ").nth(1)?.trim().to_ascii_lowercase();
        return match value.as_str() {
            "pulse" => Some(AudioDriver::Pulse),
            "alsa" => Some(AudioDriver::Alsa),
            _ => None,
        };
    }
    None
}

pub async fn read_current_driver(
    prefix_path: &str,
    runner: &ResolvedRunner,
) -> Option<AudioDriver> {
    let mut cmd = Command::new(&runner.wine_bin);
    cmd.args([
        "reg",
        "query",
        r"HKCU\Software\Wine\Drivers",
        "/v",
        "Audio",
    ])
    .env("WINEPREFIX", prefix_path)
    .env("WAYLAND_DISPLAY", "")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());

    apply_runner_env(&mut cmd, runner);

    let output = cmd.output().await.ok()?;
    if !output.status.success() {
        return None;
    }

    parse_driver_from_reg_output(&String::from_utf8_lossy(&output.stdout))
}

async fn set_audio_driver(
    prefix_path: &str,
    runner: &ResolvedRunner,
    driver: AudioDriver,
) -> Result<(), String> {
    let value = driver
        .as_reg_value()
        .ok_or_else(|| "No hay driver de audio disponible".to_string())?;

    let mut cmd = Command::new(&runner.wine_bin);
    cmd.args([
        "reg",
        "add",
        r"HKCU\Software\Wine\Drivers",
        "/v",
        "Audio",
        "/t",
        "REG_SZ",
        "/d",
        value,
        "/f",
    ])
    .env("WINEPREFIX", prefix_path)
    .env("WAYLAND_DISPLAY", "")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped());

    apply_runner_env(&mut cmd, runner);

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Error al configurar audio: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("No se pudo configurar el driver de audio: {stderr}"))
    }
}

fn emit_log(app: Option<&AppHandle>, line: impl Into<String>) {
    if let Some(app) = app {
        let _ = app.emit("ro-launcher://log", LogEvent { line: line.into() });
    }
}

pub async fn ensure_audio_driver(
    app: Option<&AppHandle>,
    prefix_path: &str,
    runner: &ResolvedRunner,
) -> Result<EnsureAudioResult, String> {
    let recommended = recommended_driver();
    let current = read_current_driver(prefix_path, runner).await;

    if recommended == AudioDriver::None {
        let message = detect_audio_backends(current).warning;
        if let Some(msg) = &message {
            emit_log(app, msg);
        }
        return Ok(EnsureAudioResult {
            configured: false,
            driver: AudioDriver::None,
            message,
        });
    }

    if recommended == AudioDriver::Pulse {
        if current == Some(AudioDriver::Alsa) {
            emit_log(
                app,
                format!("Audio: {} (configurado manualmente)", AudioDriver::Alsa.label()),
            );
            return Ok(EnsureAudioResult {
                configured: true,
                driver: AudioDriver::Alsa,
                message: None,
            });
        }

        if current.is_none() {
            set_audio_driver(prefix_path, runner, AudioDriver::Pulse).await?;
            emit_log(
                app,
                format!("Audio configurado: {}", AudioDriver::Pulse.label()),
            );
        }

        return Ok(EnsureAudioResult {
            configured: true,
            driver: AudioDriver::Pulse,
            message: None,
        });
    }

    // Pulse 32-bit no disponible: forzar ALSA para evitar mmdevapi.
    if current != Some(AudioDriver::Alsa) {
        set_audio_driver(prefix_path, runner, AudioDriver::Alsa).await?;
        let message = detect_audio_backends(Some(AudioDriver::Alsa)).warning;
        if let Some(msg) = &message {
            emit_log(app, format!("Audio configurado: ALSA (fallback). {msg}"));
        } else {
            emit_log(app, "Audio configurado: ALSA (fallback)".to_string());
        }
        return Ok(EnsureAudioResult {
            configured: true,
            driver: AudioDriver::Alsa,
            message,
        });
    }

    let message = detect_audio_backends(current).warning;
    Ok(EnsureAudioResult {
        configured: true,
        driver: AudioDriver::Alsa,
        message,
    })
}
