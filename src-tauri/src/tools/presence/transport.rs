use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceActivity {
    pub name: String,
    pub details: String,
    pub state: String,
    pub start_timestamp: Option<i64>,
    pub large_image_key: Option<String>,
    pub large_image_text: Option<String>,
}

pub trait PresenceTransport: Send {
    fn set_activity(&mut self, activity: &PresenceActivity) -> Result<(), String>;
    fn clear_activity(&mut self) -> Result<(), String>;
}

pub fn default_transport() -> Box<dyn PresenceTransport> {
    DiscordIpcTransport::from_environment()
        .map(|transport| Box::new(transport) as Box<dyn PresenceTransport>)
        .unwrap_or_else(|| Box::new(UnavailableTransport))
}

struct UnavailableTransport;

impl PresenceTransport for UnavailableTransport {
    fn set_activity(&mut self, _activity: &PresenceActivity) -> Result<(), String> {
        Err("Discord Application ID no configurado".into())
    }

    fn clear_activity(&mut self) -> Result<(), String> {
        Ok(())
    }
}

struct DiscordIpcTransport {
    application_id: String,
    process_id: u32,
    stream: Option<UnixStream>,
    nonce: u64,
}

impl DiscordIpcTransport {
    fn from_environment() -> Option<Self> {
        let application_id = std::env::var("RO_LAUNCHER_DISCORD_APPLICATION_ID")
            .ok()
            .or_else(|| std::env::var("DISCORD_APPLICATION_ID").ok())
            .or_else(|| option_env!("RO_LAUNCHER_DISCORD_APPLICATION_ID").map(str::to_owned))
            .or_else(|| option_env!("DISCORD_APPLICATION_ID").map(str::to_owned))?;
        let application_id = application_id.trim().to_string();
        if application_id.is_empty() {
            return None;
        }

        Some(Self {
            application_id,
            process_id: std::process::id(),
            stream: None,
            nonce: 0,
        })
    }

    fn connect(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            return Ok(());
        }

        let mut last_error = None;
        for path in socket_paths() {
            match UnixStream::connect(&path) {
                Ok(stream) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                    self.stream = Some(stream);
                    if let Err(error) = self.handshake() {
                        self.stream = None;
                        last_error = Some(format!("{path:?}: {error}"));
                        continue;
                    }
                    return Ok(());
                }
                Err(error) => last_error = Some(format!("{path:?}: {error}")),
            }
        }

        Err(last_error.unwrap_or_else(|| "no se encontraron sockets IPC de Discord".into()))
    }

    fn handshake(&mut self) -> Result<(), String> {
        let payload = json!({
            "v": 1,
            "client_id": self.application_id,
        });
        self.write_packet(0, &payload)?;
        let (_, response) = self.read_packet()?;
        if response.get("evt").and_then(Value::as_str) == Some("ERROR") {
            return Err(format!("handshake rechazado: {response}"));
        }
        Ok(())
    }

    fn next_nonce(&mut self) -> String {
        self.nonce = self.nonce.wrapping_add(1);
        format!("ro-launcher-{}", self.nonce)
    }

    fn send_activity(&mut self, activity: Option<Value>) -> Result<(), String> {
        self.connect()?;
        let nonce = self.next_nonce();
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": self.process_id,
                "activity": activity,
            },
            "nonce": nonce,
        });

        let result = (|| {
            self.write_packet(1, &payload)?;
            let (_, response) = self.read_packet()?;
            if response.get("evt").and_then(Value::as_str) == Some("ERROR") {
                return Err(format!("Discord rechazó SET_ACTIVITY: {response}"));
            }
            Ok(())
        })();

        if result.is_err() {
            self.stream = None;
        }
        result
    }

    fn write_packet(&mut self, opcode: u32, payload: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| "Discord IPC no está conectado".to_string())?;
        stream
            .write_all(&opcode.to_le_bytes())
            .and_then(|_| stream.write_all(&(bytes.len() as u32).to_le_bytes()))
            .and_then(|_| stream.write_all(&bytes))
            .map_err(|error| format!("no se pudo escribir Discord IPC: {error}"))
    }

    fn read_packet(&mut self) -> Result<(u32, Value), String> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| "Discord IPC no está conectado".to_string())?;
        let mut header = [0u8; 8];
        stream
            .read_exact(&mut header)
            .map_err(|error| format!("no se pudo leer Discord IPC: {error}"))?;
        let opcode = u32::from_le_bytes(header[..4].try_into().expect("opcode header"));
        let length = u32::from_le_bytes(header[4..].try_into().expect("length header"));
        if length > 1024 * 1024 {
            return Err(format!(
                "respuesta Discord IPC demasiado grande: {length} bytes"
            ));
        }
        let mut bytes = vec![0u8; length as usize];
        stream
            .read_exact(&mut bytes)
            .map_err(|error| format!("no se pudo leer el payload Discord IPC: {error}"))?;
        let payload = serde_json::from_slice(&bytes)
            .map_err(|error| format!("respuesta Discord IPC inválida: {error}"))?;
        Ok((opcode, payload))
    }
}

impl PresenceTransport for DiscordIpcTransport {
    fn set_activity(&mut self, activity: &PresenceActivity) -> Result<(), String> {
        let mut payload = json!({
            "name": activity.name,
            "type": 0,
            "details": activity.details,
            "state": activity.state,
        });
        if let Some(timestamp) = activity.start_timestamp {
            payload["timestamps"] = json!({ "start": timestamp });
        }
        if activity.large_image_key.is_some() || activity.large_image_text.is_some() {
            payload["assets"] = json!({
                "large_image": activity.large_image_key,
                "large_text": activity.large_image_text,
            });
        }
        self.send_activity(Some(payload))
    }

    fn clear_activity(&mut self) -> Result<(), String> {
        self.send_activity(None)
    }
}

fn socket_paths() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        roots.push(PathBuf::from(runtime_dir));
    }
    if let Ok(uid) = std::env::var("UID") {
        roots.push(PathBuf::from(format!("/run/user/{uid}")));
    }
    roots.push(PathBuf::from(format!("/run/user/{}", unsafe {
        libc::getuid()
    })));
    roots.push(PathBuf::from("/tmp"));

    let mut paths = Vec::new();
    for root in roots {
        for index in 0..10 {
            let path = root.join(format!("discord-ipc-{index}"));
            if !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
        }
    }
    paths
}

#[allow(dead_code)]
fn is_socket_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("discord-ipc-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_paths_include_the_runtime_directory() {
        let paths = socket_paths();
        assert!(paths.iter().any(|path| is_socket_path(path)));
    }

    #[test]
    fn activity_keeps_the_discord_friendly_fields() {
        let activity = PresenceActivity {
            name: "SakuraRO".into(),
            details: "tiny yawn · Nv. 99/70".into(),
            state: "Port Malaya".into(),
            start_timestamp: Some(123),
            large_image_key: None,
            large_image_text: None,
        };
        assert_eq!(activity.name, "SakuraRO");
        assert_eq!(activity.details, "tiny yawn · Nv. 99/70");
        assert_eq!(activity.state, "Port Malaya");
        assert_eq!(activity.start_timestamp, Some(123));
        assert!(activity.large_image_key.is_none());
    }
}
