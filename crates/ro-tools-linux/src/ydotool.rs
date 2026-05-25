use ro_tools_core::{InputWriter, ToolsError};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

/// Ruta del socket que usa ydotoold (XDG_RUNTIME_DIR o /run/user/$UID).
pub fn ydotool_socket_path() -> String {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{xdg}/.ydotool_socket")
    } else {
        format!("/run/user/{}/.ydotool_socket", current_uid())
    }
}

pub fn is_ydotool_socket_ready() -> bool {
    Path::new(&ydotool_socket_path()).exists()
}

/// Comprueba que ydotoold responde (no basta con que exista el archivo socket).
pub fn is_ydotool_responsive() -> bool {
    let path = ydotool_socket_path();
    if !Path::new(&path).exists() {
        return false;
    }

    Command::new("ydotool")
        .env("YDOTOOL_SOCKET", &path)
        .arg("type")
        .arg("")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn remove_stale_ydotool_socket() {
    let path = ydotool_socket_path();
    if Path::new(&path).exists() && !is_ydotool_responsive() {
        let _ = std::fs::remove_file(&path);
    }
}

fn binary_on_path(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn ydotool_installed() -> bool {
    binary_on_path("ydotool")
}

pub fn ydotoold_installed() -> bool {
    binary_on_path("ydotoold")
}

pub fn autopot_input_installed() -> bool {
    ydotool_installed() && ydotoold_installed()
}

pub fn current_uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn current_gid() -> u32 {
    unsafe { libc::getgid() }
}

pub struct YdotoolInput {
    socket_path: String,
}

impl YdotoolInput {
    pub fn new() -> Result<Self, ToolsError> {
        let path = ydotool_socket_path();
        if !Path::new(&path).exists() {
            return Err(ToolsError::Other(format!(
                "input virtual no disponible (socket: {path})"
            )));
        }
        Ok(Self { socket_path: path })
    }

    fn run(&self, args: &[&str]) -> Result<(), ToolsError> {
        let status = Command::new("ydotool")
            .env("YDOTOOL_SOCKET", &self.socket_path)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| ToolsError::Other(format!("ydotool: {e}")))?;

        if !status.success() {
            return Err(ToolsError::Other(format!(
                "ydotool falló: {:?}",
                status.code()
            )));
        }
        Ok(())
    }
}

impl InputWriter for YdotoolInput {
    fn press_key(&self, key: &str) -> Result<(), ToolsError> {
        let code = key_to_code(key).ok_or_else(|| ToolsError::Input {
            key: key.to_string(),
            message: "tecla no soportada".into(),
        })?;

        self.run(&["key", &format!("{code}:1")]).map_err(|e| ToolsError::Input {
            key: key.to_string(),
            message: e.to_string(),
        })?;
        thread::sleep(Duration::from_millis(15));
        self.run(&["key", &format!("{code}:0")]).map_err(|e| ToolsError::Input {
            key: key.to_string(),
            message: e.to_string(),
        })?;
        Ok(())
    }
}

/// Inicializa ydotool solo cuando hace falta potear.
pub struct LazyYdotoolInput {
    inner: Mutex<Option<YdotoolInput>>,
}

impl LazyYdotoolInput {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn reset(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }
}

impl InputWriter for LazyYdotoolInput {
    fn press_key(&self, key: &str) -> Result<(), ToolsError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| ToolsError::Other("ydotool lock poisoned".into()))?;
        if guard.is_none() {
            *guard = Some(YdotoolInput::new()?);
        }
        guard.as_ref().unwrap().press_key(key)
    }
}

fn key_to_code(key: &str) -> Option<u16> {
    match key.to_ascii_uppercase().as_str() {
        "F1" => Some(59),
        "F2" => Some(60),
        "F3" => Some(61),
        "F4" => Some(62),
        "F5" => Some(63),
        "F6" => Some(64),
        "F7" => Some(65),
        "F8" => Some(66),
        "F9" => Some(67),
        "1" => Some(2),
        "2" => Some(3),
        "3" => Some(4),
        "4" => Some(5),
        "5" => Some(6),
        "6" => Some(7),
        "7" => Some(8),
        "8" => Some(9),
        "9" => Some(10),
        "0" => Some(11),
        _ => None,
    }
}
