use serde::Serialize;
use std::path::Path;
use tauri::AppHandle;
use tokio::process::Command;

use crate::utils::emit_log_opt;
use crate::utils::ResolvedRunner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioDriver {
    Pulse,
    Alsa,
    None,
}

impl AudioDriver {
    pub fn as_str(self) -> &'static str {
        match self {
            AudioDriver::Pulse => "pulse",
            AudioDriver::Alsa => "alsa",
            AudioDriver::None => "none",
        }
    }

    pub fn as_reg_value(self) -> Option<&'static str> {
        match self {
            AudioDriver::None => None,
            driver => Some(driver.as_str()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AudioDriver::Pulse => "PulseAudio",
            AudioDriver::Alsa => "ALSA",
            AudioDriver::None => "ninguno",
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

pub fn lib32_pulse_available() -> bool {
    Path::new("/usr/lib32/libpulse.so.0").exists()
}

pub fn lib32_alsa_available() -> bool {
    Path::new("/usr/lib32/libasound.so.2").exists()
}

/// Socket Pulse expuesto por pipewire-pulse (sesión de escritorio típica en Arch/CachyOS).
pub fn pulse_session_socket_available() -> bool {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        if Path::new(&format!("{xdg}/pulse/native")).exists() {
            return true;
        }
    }
    false
}

pub fn pipewire_alsa_shim_available() -> bool {
    Path::new("/etc/alsa/conf.d/50-pipewire.conf").exists()
}

pub fn desktop_audio_session_active() -> bool {
    pulse_session_socket_available() || pipewire_alsa_shim_available()
}

pub fn audio_stack_label(driver: AudioDriver) -> &'static str {
    match driver {
        AudioDriver::None => "none",
        AudioDriver::Pulse if pulse_session_socket_available() => "pipewire",
        AudioDriver::Pulse => "pulse",
        AudioDriver::Alsa if desktop_audio_session_active() => "pipewire",
        AudioDriver::Alsa => "alsa",
    }
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

    let warning = if !ok {
        Some("Sin librerías de audio 32-bit. Instala lib32-libpulse o lib32-alsa-lib.".to_string())
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

/// Campos de audio para [`DependencyStatus`]: ok, driver serializado y aviso opcional.
pub fn dependency_audio_fields(
    prefix_path: &str,
    prefix_configured: bool,
) -> (bool, String, Option<String>, String) {
    let current_driver = if prefix_configured {
        read_current_driver_from_registry(prefix_path)
    } else {
        None
    };

    let audio_status = detect_audio_backends(current_driver);
    let audio_driver = audio_status
        .current_driver
        .or(Some(audio_status.recommended))
        .unwrap_or(AudioDriver::None);
    let stack = audio_stack_label(audio_driver).to_string();

    (
        audio_status.ok,
        audio_driver.as_str().to_string(),
        audio_status.warning,
        stack,
    )
}

fn read_current_driver_from_registry(prefix_path: &str) -> Option<AudioDriver> {
    let registry = std::fs::read_to_string(Path::new(prefix_path).join("user.reg")).ok()?;
    let mut in_drivers = false;
    for line in registry.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_drivers = trimmed.starts_with(r"[Software\\Wine\\Drivers]");
            continue;
        }
        if !in_drivers {
            continue;
        }
        let Some(value) = trimmed.strip_prefix(r#""Audio"="#) else {
            continue;
        };
        let value = value.trim_matches('"');
        return match value.to_ascii_lowercase().as_str() {
            "pulse" => Some(AudioDriver::Pulse),
            "alsa" => Some(AudioDriver::Alsa),
            _ => None,
        };
    }
    None
}

pub fn is_mmdevapi_audio_error(line: &str) -> bool {
    line.contains("err:mmdevapi")
        && (line.contains("load_driver") || line.contains("DllGetClassObject"))
}

pub fn mmdevapi_recovery_hint() -> &'static str {
    "Fallo de audio detectado. Revisa el driver mostrado en Avanzado. \
     En Arch/CachyOS instala las bibliotecas de 32 bits: sudo pacman -S lib32-libpulse lib32-alsa-lib"
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

fn wine_reg_command(runner: &ResolvedRunner, prefix_path: &str, args: &[&str]) -> Command {
    runner.builtin_command(prefix_path, "reg", args.iter().copied())
}

pub async fn read_current_driver(
    prefix_path: &str,
    runner: &ResolvedRunner,
) -> Option<AudioDriver> {
    let output = wine_reg_command(
        runner,
        prefix_path,
        &["query", r"HKCU\Software\Wine\Drivers", "/v", "Audio"],
    )
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .ok()?;
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

    let output = wine_reg_command(
        runner,
        prefix_path,
        &[
            "add",
            r"HKCU\Software\Wine\Drivers",
            "/v",
            "Audio",
            "/t",
            "REG_SZ",
            "/d",
            value,
            "/f",
        ],
    )
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped())
    .output()
    .await
    .map_err(|e| format!("Error al configurar audio: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "No se pudo configurar el driver de audio: {stderr}"
        ))
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
            emit_log_opt(app, msg);
        }
        return Ok(EnsureAudioResult {
            configured: false,
            driver: AudioDriver::None,
            message,
        });
    }

    if recommended == AudioDriver::Pulse {
        if current == Some(AudioDriver::Alsa) {
            emit_log_opt(
                app,
                format!(
                    "Audio: {} (configurado manualmente)",
                    AudioDriver::Alsa.label()
                ),
            );
            return Ok(EnsureAudioResult {
                configured: true,
                driver: AudioDriver::Alsa,
                message: None,
            });
        }

        if current.is_none() {
            set_audio_driver(prefix_path, runner, AudioDriver::Pulse).await?;
            emit_log_opt(
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
        let log_label = if desktop_audio_session_active() {
            "Audio configurado: ALSA (PipeWire)"
        } else {
            "Audio configurado: ALSA"
        };
        emit_log_opt(app, log_label.to_string());
        return Ok(EnsureAudioResult {
            configured: true,
            driver: AudioDriver::Alsa,
            message: None,
        });
    }

    Ok(EnsureAudioResult {
        configured: true,
        driver: AudioDriver::Alsa,
        message: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alsa_only_backend_is_ok_without_warning() {
        let status = detect_audio_backends(None);
        if lib32_alsa_available() && !lib32_pulse_available() {
            assert!(status.ok);
            assert!(status.warning.is_none());
            assert_eq!(status.recommended, AudioDriver::Alsa);
        }
    }

    #[test]
    fn missing_libs_is_not_ok() {
        if lib32_alsa_available() || lib32_pulse_available() {
            return;
        }
        let status = detect_audio_backends(None);
        assert!(!status.ok);
        assert!(status.warning.is_some());
    }

    #[test]
    fn parses_audio_driver_from_user_registry_without_running_wine() {
        let dir =
            std::env::temp_dir().join(format!("ro-launcher-audio-registry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("user.reg"),
            "WINE REGISTRY Version 2\n\n[Software\\\\Wine\\\\Drivers] 1\n\"Audio\"=\"pulse\"\n",
        )
        .unwrap();
        assert_eq!(
            read_current_driver_from_registry(dir.to_str().unwrap()),
            Some(AudioDriver::Pulse)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stack_label_marks_pipewire_alsa_on_desktop() {
        if !desktop_audio_session_active() {
            return;
        }
        assert_eq!(audio_stack_label(AudioDriver::Alsa), "pipewire");
    }
}
