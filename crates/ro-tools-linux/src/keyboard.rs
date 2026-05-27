use evdev::{uinput::VirtualDevice, AttributeSet, Device, InputEvent, KeyCode};
use ro_tools_core::ToolsError;

/// Lee teclados físicos vía evdev para detectar hold (estilo 4RTools `Keyboard.IsKeyDown`).
pub struct KeyboardMonitor {
    devices: Vec<Device>,
}

impl KeyboardMonitor {
    pub fn open() -> Result<Self, ToolsError> {
        let mut devices = Vec::new();

        for (_path, dev) in evdev::enumerate() {
            let Some(keys) = dev.supported_keys() else {
                continue;
            };

            if !keys.contains(KeyCode::KEY_F1) {
                continue;
            }

            let _ = dev.set_nonblocking(true);
            devices.push(dev);
        }

        if devices.is_empty() {
            return Err(ToolsError::Other(
                "No se pudo leer teclados evdev (¿grupo input? sudo usermod -aG input $USER)"
                    .into(),
            ));
        }

        Ok(Self { devices })
    }

    /// Actualiza el estado interno leyendo eventos pendientes.
    pub fn sync(&mut self) {
        for dev in &mut self.devices {
            if let Ok(events) = dev.fetch_events() {
                for _ in events {}
            }
        }
    }

    /// Lee eventos raw de todos los dispositivos y los divide en:
    /// - trigger_down: si el trigger key pasó a pressed en este poll
    /// - trigger_up: si el trigger key fue soltado en este poll
    /// - passthrough: resto de eventos (otras teclas + EV_SYN) para reemitir
    pub fn read_and_split(&mut self, trigger_key: &str) -> (bool, bool, Vec<InputEvent>) {
        let trigger_code = label_to_keycode(trigger_key);
        let mut trigger_down = false;
        let mut trigger_up = false;
        let mut passthrough: Vec<InputEvent> = Vec::new();

        for dev in &mut self.devices {
            if let Ok(events) = dev.fetch_events() {
                for event in events {
                    if let evdev::EventSummary::Key(_, code, value) = event.destructure() {
                        if trigger_code == Some(code) {
                            match value {
                                1 => trigger_down = true,
                                0 => trigger_up = true,
                                _ => {} // 2 = auto-repeat del kernel, ignorar
                            }
                            continue; // no forwarded
                        }
                    }
                    passthrough.push(event);
                }
            }
        }

        (trigger_down, trigger_up, passthrough)
    }

    pub fn is_key_down(&self, label: &str) -> bool {
        let Some(code) = label_to_keycode(label) else {
            return false;
        };

        self.devices.iter().any(|dev| {
            dev.cached_state()
                .key_vals()
                .map(|keys| keys.contains(code))
                .unwrap_or(false)
        })
    }

    /// Grabs all keyboard devices — compositor/Wine dejan de ver el teclado físico.
    /// Requiere grupo `input`. Falla si otro proceso ya tiene el grab.
    pub fn grab(&mut self) -> Result<(), ro_tools_core::ToolsError> {
        for dev in &mut self.devices {
            dev.grab()
                .map_err(|e| ro_tools_core::ToolsError::Other(format!("evdev grab: {e}")))?;
        }
        Ok(())
    }

    /// Libera el grab en todos los dispositivos (best-effort).
    pub fn ungrab(&mut self) {
        use std::os::unix::io::AsRawFd;
        for dev in &mut self.devices {
            // EVIOCGRAB(0) — evdev 0.13 no expone ungrab() en API pública
            unsafe { libc::ioctl(dev.as_raw_fd(), 0x4004_4590u64, 0i32) };
        }
    }

    pub fn is_alt_down(&self) -> bool {
        self.is_code_down(KeyCode::KEY_LEFTALT) || self.is_code_down(KeyCode::KEY_RIGHTALT)
    }

    fn is_code_down(&self, code: KeyCode) -> bool {
        self.devices.iter().any(|dev| {
            dev.cached_state()
                .key_vals()
                .map(|keys| keys.contains(code))
                .unwrap_or(false)
        })
    }
}

/// Dispositivo uinput que recibe eventos del teclado físico grabado y los
/// re-emite al compositor, actuando como passthrough transparente.
pub struct KeyboardPassthrough {
    device: evdev::uinput::VirtualDevice,
}

impl KeyboardPassthrough {
    pub fn new(template: &KeyboardMonitor) -> Result<Self, ToolsError> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for phys_dev in &template.devices {
            if let Some(supported) = phys_dev.supported_keys() {
                for key in supported.iter() {
                    keys.insert(key);
                }
            }
        }

        if keys.iter().next().is_none() {
            return Err(ToolsError::Other("teclado sin info de teclas".into()));
        }

        let device = VirtualDevice::builder()
            .map_err(|e| ToolsError::Other(format!("uinput init: {e}")))?
            .name("ro-launcher-kb-passthrough")
            .with_keys(&keys)
            .map_err(|e| ToolsError::Other(format!("uinput keys: {e}")))?
            .build()
            .map_err(|e| ToolsError::Other(format!("uinput build: {e}")))?;

        Ok(Self { device })
    }

    pub fn emit(&mut self, events: &[InputEvent]) -> Result<(), ToolsError> {
        self.device
            .emit(events)
            .map_err(|e| ToolsError::Other(format!("uinput emit: {e}")))
    }
}

pub fn key_label_to_keycode(label: &str) -> Option<KeyCode> {
    label_to_keycode(label)
}

fn label_to_keycode(label: &str) -> Option<KeyCode> {
    Some(match label.to_ascii_uppercase().as_str() {
        "F1" => KeyCode::KEY_F1,
        "F2" => KeyCode::KEY_F2,
        "F3" => KeyCode::KEY_F3,
        "F4" => KeyCode::KEY_F4,
        "F5" => KeyCode::KEY_F5,
        "F6" => KeyCode::KEY_F6,
        "F7" => KeyCode::KEY_F7,
        "F8" => KeyCode::KEY_F8,
        "F9" => KeyCode::KEY_F9,
        "1" => KeyCode::KEY_1,
        "2" => KeyCode::KEY_2,
        "3" => KeyCode::KEY_3,
        "4" => KeyCode::KEY_4,
        "5" => KeyCode::KEY_5,
        "6" => KeyCode::KEY_6,
        "7" => KeyCode::KEY_7,
        "8" => KeyCode::KEY_8,
        "9" => KeyCode::KEY_9,
        "0" => KeyCode::KEY_0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_f_keys() {
        assert!(label_to_keycode("F1").is_some());
        assert!(label_to_keycode("f9").is_some());
        assert!(label_to_keycode("ZZ").is_none());
    }

    #[test]
    fn open_does_not_panic_without_devices() {
        let _ = KeyboardMonitor::open();
    }
}
