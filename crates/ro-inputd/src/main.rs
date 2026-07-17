use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, InputEvent, KeyCode};
use ro_tools_core::spammer::keys::normalize_spammer_keys;
use ro_tools_linux::key_label_to_keycode;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

const BLACKLIST: &[&str] = &[
    "mouse",
    "ro-launcher",
    "uinput",
    "power",
    "mic",
    "headset",
    "speakerphone",
    "volume",
    "webcam",
    "camera",
];

struct DiscoveredDevice {
    path: PathBuf,
    device: Device,
}

struct TriggerSet {
    labels: Vec<String>,
    code_to_label: HashMap<KeyCode, String>,
}

struct Config {
    triggers: TriggerSet,
    json: bool,
}

fn parse_args() -> Config {
    let mut trigger_labels: Vec<String> = Vec::new();
    let mut json = false;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--trigger" if i + 1 < args.len() => {
                trigger_labels.push(args[i + 1].clone());
                i += 2;
            }
            "--triggers" if i + 1 < args.len() => {
                for part in args[i + 1].split(',') {
                    trigger_labels.push(part.trim().to_string());
                }
                i += 2;
            }
            "--json" => {
                json = true;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if trigger_labels.is_empty() {
        trigger_labels.push("F1".into());
    }

    let labels = normalize_spammer_keys(&trigger_labels);
    let mut code_to_label = HashMap::new();
    for label in &labels {
        if let Some(code) = key_label_to_keycode(label) {
            code_to_label.insert(code, label.clone());
        }
    }

    Config {
        triggers: TriggerSet {
            labels,
            code_to_label,
        },
        json,
    }
}

fn is_excluded(dev: &Device) -> bool {
    let name = dev.name().unwrap_or("").to_lowercase();
    BLACKLIST.iter().any(|b| name.contains(b))
}

fn is_mouse_handler(path: &Path) -> bool {
    path.to_string_lossy().to_lowercase().contains("mouse")
}

fn has_rel_axes(dev: &Device) -> bool {
    dev.supported_relative_axes()
        .map(|a| a.iter().next().is_some())
        .unwrap_or(false)
}

fn has_abs_axes(dev: &Device) -> bool {
    dev.supported_absolute_axes()
        .map(|a| a.iter().next().is_some())
        .unwrap_or(false)
}

fn phys_group_key(dev: &Device, path: &Path) -> String {
    dev.physical_path()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn discover_keyboards(triggers: &HashMap<KeyCode, String>) -> Vec<DiscoveredDevice> {
    let mut candidates = Vec::new();
    for (path, dev) in evdev::enumerate() {
        let Some(keys) = dev.supported_keys() else {
            continue;
        };
        if !triggers.keys().any(|code| keys.contains(*code)) {
            continue;
        }
        if has_rel_axes(&dev) || has_abs_axes(&dev) {
            continue;
        }
        if is_mouse_handler(&path) || is_excluded(&dev) {
            continue;
        }
        let _ = dev.set_nonblocking(true);
        candidates.push(DiscoveredDevice { path, device: dev });
    }

    let mut groups: HashMap<String, Vec<DiscoveredDevice>> = HashMap::new();
    for cand in candidates {
        let key = phys_group_key(&cand.device, &cand.path);
        groups.entry(key).or_default().push(cand);
    }

    groups.into_values().flatten().collect()
}

fn create_passthrough(devices: &[Device]) -> Result<VirtualDevice, String> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for dev in devices {
        if let Some(supported) = dev.supported_keys() {
            for key in supported.iter() {
                keys.insert(key);
            }
        }
    }

    VirtualDevice::builder()
        .map_err(|e| format!("uinput builder: {e}"))?
        .name("ro-launcher-kb-passthrough")
        .with_keys(&keys)
        .map_err(|e| format!("uinput with_keys: {e}"))?
        .build()
        .map_err(|e| format!("uinput build: {e}"))
}

fn ungrab(dev: &mut Device) {
    // EVIOCGRAB(0): evdev 0.13 no expone ungrab en API pública
    unsafe { libc::ioctl(dev.as_raw_fd(), 0x4004_4590u64, 0i32) };
}

fn emit_json(val: &serde_json::Value) {
    let line = serde_json::to_string(val).unwrap_or_default();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn any_alt_held(devices: &[Device]) -> bool {
    devices.iter().any(|dev| {
        dev.cached_state()
            .key_vals()
            .map(|keys| keys.contains(KeyCode::KEY_LEFTALT) || keys.contains(KeyCode::KEY_RIGHTALT))
            .unwrap_or(false)
    })
}

fn code_held(devices: &[Device], code: KeyCode) -> bool {
    devices.iter().any(|dev| {
        dev.cached_state()
            .key_vals()
            .map(|keys| keys.contains(code))
            .unwrap_or(false)
    })
}

fn trigger_uses_alt_passthrough(
    code: KeyCode,
    value: i32,
    alt_held: bool,
    passthrough_keys: &mut HashSet<KeyCode>,
) -> bool {
    match value {
        1 if alt_held => passthrough_keys.insert(code),
        2 => passthrough_keys.contains(&code),
        0 => passthrough_keys.remove(&code),
        _ => false,
    }
}

fn label_for_code(triggers: &TriggerSet, code: KeyCode) -> Option<&str> {
    triggers.code_to_label.get(&code).map(String::as_str)
}

fn create_shutdown_eventfd() -> io::Result<i32> {
    let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

fn wake_shutdown(fd: i32) {
    let value: u64 = 1;
    unsafe {
        libc::write(
            fd,
            (&value as *const u64).cast::<libc::c_void>(),
            std::mem::size_of::<u64>(),
        );
    }
}

/// Blocks until an evdev descriptor is readable or shutdown is signalled.
/// Returns false for shutdown and true for input activity.
fn wait_for_events(device_fds: &[i32], shutdown_fd: i32) -> io::Result<bool> {
    let mut fds = Vec::with_capacity(device_fds.len() + 1);
    fds.push(libc::pollfd {
        fd: shutdown_fd,
        events: libc::POLLIN,
        revents: 0,
    });
    fds.extend(device_fds.iter().map(|fd| libc::pollfd {
        fd: *fd,
        events: libc::POLLIN,
        revents: 0,
    }));

    loop {
        let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if result >= 0 {
            return Ok(fds[0].revents & libc::POLLIN == 0);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn main() {
    let cfg = parse_args();
    if cfg.triggers.code_to_label.is_empty() {
        if cfg.json {
            emit_json(&serde_json::json!({
                "type": "fatal",
                "message": "Ninguna tecla trigger válida (usa F1-F9, 0-9 o A-Z)"
            }));
        }
        return;
    }

    let discovered = discover_keyboards(&cfg.triggers.code_to_label);
    if discovered.is_empty() {
        if cfg.json {
            emit_json(&serde_json::json!({
                "type": "fatal",
                "message": "No se encontraron teclados (¿grupo input? sudo usermod -aG input $USER)"
            }));
        }
        return;
    }

    let device_paths: Vec<String> = discovered
        .iter()
        .map(|d| d.path.to_string_lossy().into_owned())
        .collect();
    let device_names: Vec<String> = discovered
        .iter()
        .filter_map(|d| d.device.name().map(str::to_string))
        .collect();

    let devices_only: Vec<Device> = discovered.into_iter().map(|d| d.device).collect();

    let mut passthrough = match create_passthrough(&devices_only) {
        Ok(pt) => pt,
        Err(e) => {
            if cfg.json {
                emit_json(&serde_json::json!({"type": "fatal", "message": e}));
            }
            return;
        }
    };

    let mut devices = devices_only;
    let mut grabbed = 0usize;
    for dev in &mut devices {
        match dev.grab() {
            Ok(()) => grabbed += 1,
            Err(e) => {
                for d in devices.iter_mut().take(grabbed) {
                    ungrab(d);
                }
                if cfg.json {
                    emit_json(
                        &serde_json::json!({"type": "fatal", "message": format!("grab: {e}")}),
                    );
                }
                return;
            }
        }
    }

    if cfg.json {
        emit_json(&serde_json::json!({
            "type": "ready",
            "devices": device_paths,
            "name": device_names.first().cloned().unwrap_or_else(|| "unknown".into()),
            "triggers": cfg.triggers.labels,
        }));
    }

    let shutdown_fd = match create_shutdown_eventfd() {
        Ok(fd) => fd,
        Err(error) => {
            if cfg.json {
                emit_json(&serde_json::json!({
                    "type": "fatal",
                    "message": format!("eventfd shutdown: {error}")
                }));
            }
            for dev in &mut devices {
                ungrab(dev);
            }
            return;
        }
    };

    let shutdown_stdin_fd = shutdown_fd;
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) if l.contains("\"stop\"") => break,
                Err(_) => break,
                _ => {}
            }
        }
        wake_shutdown(shutdown_stdin_fd);
    });

    let shutdown_signal_fd = shutdown_fd;
    std::thread::spawn(move || {
        let mut signals =
            signal_hook::iterator::Signals::new([libc::SIGTERM, libc::SIGINT]).expect("signals");
        if signals.forever().next().is_some() {
            wake_shutdown(shutdown_signal_fd);
        }
    });

    let mut left_alt_held = code_held(&devices, KeyCode::KEY_LEFTALT);
    let mut right_alt_held = code_held(&devices, KeyCode::KEY_RIGHTALT);
    let mut alt_held = any_alt_held(&devices);
    let mut alt_passthrough_keys = HashSet::new();
    let device_fds: Vec<i32> = devices.iter().map(AsRawFd::as_raw_fd).collect();

    loop {
        match wait_for_events(&device_fds, shutdown_fd) {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                if cfg.json {
                    emit_json(&serde_json::json!({
                        "type":"fatal",
                        "message": format!("poll evdev: {error}")
                    }));
                }
                break;
            }
        }

        let mut had_error = false;
        for dev in &mut devices {
            match dev.fetch_events() {
                Ok(events) => {
                    let mut passthrough_buf: Vec<InputEvent> = Vec::new();
                    for event in events {
                        if let evdev::EventSummary::Key(_, code, value) = event.destructure() {
                            if code == KeyCode::KEY_LEFTALT || code == KeyCode::KEY_RIGHTALT {
                                if code == KeyCode::KEY_LEFTALT {
                                    left_alt_held = value != 0;
                                } else {
                                    right_alt_held = value != 0;
                                }
                                alt_held = left_alt_held || right_alt_held;
                            }
                            if cfg.triggers.code_to_label.contains_key(&code) {
                                let Some(label) = label_for_code(&cfg.triggers, code) else {
                                    continue;
                                };
                                if trigger_uses_alt_passthrough(
                                    code,
                                    value,
                                    alt_held,
                                    &mut alt_passthrough_keys,
                                ) {
                                    if value == 1 && cfg.json {
                                        emit_json(&serde_json::json!({
                                            "type":"trigger",
                                            "key": label,
                                            "held": false,
                                        }));
                                    }
                                    passthrough_buf.push(event);
                                    continue;
                                }
                                match value {
                                    1 => {
                                        if cfg.json {
                                            emit_json(&serde_json::json!({
                                                "type":"trigger",
                                                "key": label,
                                                "held": !alt_held,
                                            }));
                                        }
                                    }
                                    0 if cfg.json => {
                                        emit_json(&serde_json::json!({
                                            "type":"trigger",
                                            "key": label,
                                            "held": false,
                                        }));
                                    }
                                    _ => {} // auto-repeat, ignorar
                                }
                                continue;
                            }
                        }
                        passthrough_buf.push(event);
                    }
                    if !passthrough_buf.is_empty() {
                        let _ = passthrough.emit(&passthrough_buf);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    if cfg.json {
                        emit_json(&serde_json::json!({"type":"fatal","message": e.to_string()}));
                    }
                    had_error = true;
                    break;
                }
            }
        }

        if had_error {
            break;
        }
    }

    for dev in &mut devices {
        ungrab(dev);
    }
    unsafe { libc::close(shutdown_fd) };
    if cfg.json {
        emit_json(&serde_json::json!({"type":"shutdown"}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blacklist_rejects_mouse_names() {
        let bad_names = [
            "USB Mouse",
            "Logitech G502 HERO GAMING MOUSE",
            "HDA Intel MIC",
        ];
        for name in &bad_names {
            let lower = name.to_lowercase();
            let excluded = BLACKLIST.iter().any(|b| lower.contains(b));
            assert!(excluded, "'{name}' should be excluded");
        }
    }

    #[test]
    fn blacklist_allows_keyboard_names() {
        let good_names = [
            "AT Translated Set 2 keyboard",
            "Logitech G Pro Mechanical Gaming Keyboard",
            "USB Keyboard",
        ];
        for name in &good_names {
            let lower = name.to_lowercase();
            let excluded = BLACKLIST.iter().any(|b| lower.contains(b));
            assert!(!excluded, "'{name}' should NOT be excluded");
        }
    }

    #[test]
    fn rejects_mouse_handler_paths() {
        assert!(is_mouse_handler(Path::new(
            "/dev/input/by-id/usb-mouse-event-mouse"
        )));
        assert!(!is_mouse_handler(Path::new("/dev/input/event6")));
    }

    #[test]
    fn eventfd_wakes_blocked_poll_for_shutdown() {
        let fd = create_shutdown_eventfd().unwrap();
        let wake_fd = fd;
        let wake = std::thread::spawn(move || wake_shutdown(wake_fd));
        assert!(!wait_for_events(&[], fd).unwrap());
        wake.join().unwrap();
        unsafe { libc::close(fd) };
    }

    #[test]
    fn alt_trigger_passthrough_keeps_down_repeat_up_sequence() {
        let mut keys = HashSet::new();
        assert!(trigger_uses_alt_passthrough(
            KeyCode::KEY_F2,
            1,
            true,
            &mut keys
        ));
        assert!(trigger_uses_alt_passthrough(
            KeyCode::KEY_F2,
            2,
            false,
            &mut keys
        ));
        assert!(trigger_uses_alt_passthrough(
            KeyCode::KEY_F2,
            0,
            false,
            &mut keys
        ));
        assert!(!keys.contains(&KeyCode::KEY_F2));
    }
}
