use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use evdev::{
    uinput::VirtualDevice, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode,
};
use ro_tools_core::ToolsError;

use crate::keyboard::key_label_to_keycode;

pub const COMBAT_DEVICE_NAME: &str = "ro-launcher-combat-input";

/// Persistent combined keyboard/mouse device used by the combat-input worker.
/// Keeping both event classes on one evdev node preserves their emission order.
pub struct CombatUinput {
    device: VirtualDevice,
    nodes: Vec<PathBuf>,
    pressed_keys: HashSet<KeyCode>,
    mouse_left_pressed: bool,
}

impl CombatUinput {
    pub fn create() -> Result<Self, ToolsError> {
        let mut keys = supported_combat_keys();
        keys.insert(KeyCode::BTN_LEFT);
        let mouse_axes = supported_pointer_axes();
        let mut device = VirtualDevice::builder()
            .map_err(|error| uinput_error("open", COMBAT_DEVICE_NAME, error))?
            .name(COMBAT_DEVICE_NAME)
            .with_keys(&keys)
            .map_err(|error| uinput_error("configure keys/buttons", COMBAT_DEVICE_NAME, error))?
            .with_relative_axes(&mouse_axes)
            .map_err(|error| uinput_error("configure axes", COMBAT_DEVICE_NAME, error))?
            .build()
            .map_err(|error| uinput_error("create", COMBAT_DEVICE_NAME, error))?;

        // This blocks until udev/sysfs has exposed the corresponding event nodes.
        let nodes = enumerate_nodes(&mut device, COMBAT_DEVICE_NAME)?;

        Ok(Self {
            device,
            nodes,
            pressed_keys: HashSet::new(),
            mouse_left_pressed: false,
        })
    }

    pub fn device_summary(&self) -> String {
        format!("combined={}", display_nodes(&self.nodes))
    }

    pub fn key_event(&mut self, key: &str, value: i32) -> Result<(), ToolsError> {
        let code = key_label_to_keycode(key).ok_or_else(|| ToolsError::Input {
            key: key.to_string(),
            message: "tecla no soportada por uinput".into(),
        })?;
        let result = self
            .device
            .emit(&[InputEvent::new(EventType::KEY.0, code.0, value)])
            .map_err(|error| uinput_error("write key", COMBAT_DEVICE_NAME, error));
        if result.is_ok() {
            update_key_state(&mut self.pressed_keys, code, value);
        }
        result
    }

    pub fn mouse_left_event(&mut self, value: i32) -> Result<(), ToolsError> {
        let result = self
            .device
            .emit(&[InputEvent::new(
                EventType::KEY.0,
                KeyCode::BTN_LEFT.0,
                value,
            )])
            .map_err(|error| uinput_error("write button", COMBAT_DEVICE_NAME, error));
        if result.is_ok() {
            self.mouse_left_pressed = value != 0;
        }
        result
    }

    pub fn release(&mut self, key: Option<&str>, mouse_left: bool) {
        if mouse_left || self.mouse_left_pressed {
            let _ = self.mouse_left_event(0);
        }
        match key {
            Some(key) => {
                let _ = self.key_event(key, 0);
            }
            None => {
                for code in self.pressed_keys.clone() {
                    if self
                        .device
                        .emit(&[InputEvent::new(EventType::KEY.0, code.0, 0)])
                        .is_ok()
                    {
                        self.pressed_keys.remove(&code);
                    }
                }
            }
        }
    }
}

impl Drop for CombatUinput {
    fn drop(&mut self) {
        self.release(None, true);
    }
}

fn update_key_state(pressed: &mut HashSet<KeyCode>, code: KeyCode, value: i32) {
    match value {
        0 => {
            pressed.remove(&code);
        }
        1 => {
            pressed.insert(code);
        }
        _ => {}
    }
}

fn enumerate_nodes(
    device: &mut VirtualDevice,
    device_name: &str,
) -> Result<Vec<PathBuf>, ToolsError> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let (last_errno, last_error) = match device.enumerate_dev_nodes_blocking() {
            Ok(nodes) => match nodes.collect::<io::Result<Vec<_>>>() {
                Ok(nodes) if !nodes.is_empty() => return Ok(nodes),
                Ok(_) => ("none".into(), "no event node appeared".into()),
                Err(error) => (errno_string(&error), error.to_string()),
            },
            Err(error) => (errno_string(&error), error.to_string()),
        };
        if Instant::now() >= deadline {
            return Err(ToolsError::Other(format!(
                "uinput stage=wait sysfs device={device_name} errno={last_errno}: {last_error}"
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn display_nodes(nodes: &[PathBuf]) -> String {
    nodes
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn uinput_error(stage: &str, device: &str, error: io::Error) -> ToolsError {
    let errno = errno_string(&error);
    ToolsError::Other(format!(
        "uinput stage={stage} device={device} errno={errno}: {error}"
    ))
}

fn errno_string(error: &io::Error) -> String {
    error
        .raw_os_error()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn supported_combat_keys() -> AttributeSet<KeyCode> {
    const LABELS: &[&str] = &[
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "1", "2", "3",
        "4", "5", "6", "7", "8", "9", "0", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "A",
        "S", "D", "F", "G", "H", "J", "K", "L", "Z", "X", "C", "V", "B", "N", "M",
    ];
    AttributeSet::from_iter(
        LABELS
            .iter()
            .filter_map(|label| key_label_to_keycode(label)),
    )
}

fn supported_pointer_axes() -> AttributeSet<RelativeAxisCode> {
    AttributeSet::from_iter([RelativeAxisCode::REL_X, RelativeAxisCode::REL_Y])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_device_supports_all_configurable_keys() {
        let mut keys = supported_combat_keys();
        keys.insert(KeyCode::BTN_LEFT);
        for label in ["F1", "F12", "0", "Q", "M"] {
            assert!(
                keys.contains(key_label_to_keycode(label).unwrap()),
                "{label}"
            );
        }
        assert!(keys.contains(KeyCode::BTN_LEFT));
        let axes = supported_pointer_axes();
        assert!(axes.contains(RelativeAxisCode::REL_X));
        assert!(axes.contains(RelativeAxisCode::REL_Y));
    }

    #[test]
    fn diagnostics_include_stage_device_and_errno() {
        let message = uinput_error(
            "create",
            COMBAT_DEVICE_NAME,
            io::Error::from_raw_os_error(libc::EACCES),
        )
        .to_string();
        assert!(message.contains("stage=create"));
        assert!(message.contains(COMBAT_DEVICE_NAME));
        assert!(message.contains("errno=13"));
    }

    #[test]
    fn pressed_key_state_survives_repeats_and_clears_on_release() {
        let mut pressed = HashSet::new();
        update_key_state(&mut pressed, KeyCode::KEY_F1, 1);
        update_key_state(&mut pressed, KeyCode::KEY_F1, 2);
        assert!(pressed.contains(&KeyCode::KEY_F1));

        update_key_state(&mut pressed, KeyCode::KEY_F1, 0);
        assert!(pressed.is_empty());
    }

    #[test]
    #[ignore = "requires Linux /dev/uinput access"]
    fn linux_combined_device_exposes_keyboard_and_pointer_on_one_node() {
        let input = CombatUinput::create().unwrap();
        assert_eq!(input.nodes.len(), 1, "expected one combined evdev node");

        let deadline = Instant::now() + Duration::from_secs(1);
        let device = loop {
            match evdev::Device::open(&input.nodes[0]) {
                Ok(device) => break device,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("could not inspect combined evdev node: {error}"),
            }
        };
        let keys = device.supported_keys().unwrap();
        assert!(keys.contains(KeyCode::KEY_F2));
        assert!(keys.contains(KeyCode::BTN_LEFT));
        let axes = device.supported_relative_axes().unwrap();
        assert!(axes.contains(RelativeAxisCode::REL_X));
        assert!(axes.contains(RelativeAxisCode::REL_Y));
    }
}
