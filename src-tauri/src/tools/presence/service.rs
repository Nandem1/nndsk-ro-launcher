use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ro_tools_core::{
    read_character_snapshot, CharacterSnapshot, CharacterState, PresenceMemoryProfile,
};
use ro_tools_linux::{address_in_maps, verify_process_identity, ProcMemoryReader, ProcessIdentity};

use super::map_names::display_map_name;
use super::profiles::resolve_profile;
use super::transport::{default_transport, PresenceActivity, PresenceTransport};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const REQUIRED_STABLE_SAMPLES: u8 = 2;
const MAX_INVALID_SAMPLES: u8 = 3;

#[derive(Clone)]
pub struct PresenceHandle {
    commands: Sender<PresenceCommand>,
    worker: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl PresenceHandle {
    pub fn new() -> Self {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("ro-presence".into())
            .spawn(move || run_worker(receiver, default_transport()))
            .expect("no se pudo iniciar el worker de Discord Rich Presence");
        Self {
            commands,
            worker: Arc::new(Mutex::new(Some(worker))),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.commands.send(PresenceCommand::SetEnabled(enabled));
    }

    pub fn register(
        &self,
        client_id: String,
        server_name: String,
        identity: ProcessIdentity,
        executable_path: String,
    ) {
        let _ = self.commands.send(PresenceCommand::Register {
            client_id,
            server_name,
            identity,
            executable_path,
        });
    }

    pub fn handoff(&self, client_id: &str, identity: ProcessIdentity) {
        let _ = self.commands.send(PresenceCommand::Handoff {
            client_id: client_id.to_string(),
            identity,
        });
    }

    pub fn unregister(&self, client_id: &str) {
        let _ = self.commands.send(PresenceCommand::Unregister {
            client_id: client_id.to_string(),
        });
    }

    pub fn shutdown(&self) {
        let Ok(mut worker) = self.worker.lock() else {
            return;
        };
        let Some(worker) = worker.take() else {
            return;
        };
        let _ = self.commands.send(PresenceCommand::Shutdown);
        let _ = worker.join();
    }
}

impl Default for PresenceHandle {
    fn default() -> Self {
        Self::new()
    }
}

enum PresenceCommand {
    SetEnabled(bool),
    Register {
        client_id: String,
        server_name: String,
        identity: ProcessIdentity,
        executable_path: String,
    },
    Handoff {
        client_id: String,
        identity: ProcessIdentity,
    },
    Unregister {
        client_id: String,
    },
    Shutdown,
}

struct PresenceWorker {
    receiver: Receiver<PresenceCommand>,
    transport: Box<dyn PresenceTransport>,
    enabled: bool,
    clients: HashMap<String, TrackedClient>,
    sent_activity: Option<PresenceActivity>,
    last_publish: Option<Instant>,
    last_error: Option<String>,
}

struct TrackedClient {
    server_name: String,
    identity: ProcessIdentity,
    profile: Option<PresenceMemoryProfile>,
    session_started: i64,
    character_name: Option<String>,
    snapshot: Option<CharacterSnapshot>,
    pending: Option<PendingSnapshot>,
    invalid_samples: u8,
}

struct PendingSnapshot {
    key: SnapshotKey,
    snapshot: CharacterSnapshot,
    samples: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotKey {
    character_name: Option<String>,
    level: Option<u32>,
    job_level: Option<u32>,
    map_name: Option<String>,
}

impl PresenceWorker {
    fn new(receiver: Receiver<PresenceCommand>, transport: Box<dyn PresenceTransport>) -> Self {
        Self {
            receiver,
            transport,
            enabled: false,
            clients: HashMap::new(),
            sent_activity: None,
            last_publish: None,
            last_error: None,
        }
    }

    fn handle(&mut self, command: PresenceCommand) -> bool {
        match command {
            PresenceCommand::SetEnabled(enabled) => {
                self.enabled = enabled;
                if !enabled {
                    self.publish(true);
                }
            }
            PresenceCommand::Register {
                client_id,
                server_name,
                identity,
                executable_path,
            } => {
                let profile = resolve_profile(&executable_path);
                self.clients.insert(
                    client_id,
                    TrackedClient {
                        server_name,
                        identity,
                        profile,
                        session_started: unix_timestamp(),
                        character_name: None,
                        snapshot: None,
                        pending: None,
                        invalid_samples: 0,
                    },
                );
                self.sample_clients();
                self.publish(true);
            }
            PresenceCommand::Handoff {
                client_id,
                identity,
            } => {
                if let Some(client) = self.clients.get_mut(&client_id) {
                    client.identity = identity;
                    client.snapshot = None;
                    client.pending = None;
                    client.invalid_samples = 0;
                }
                self.sample_clients();
                self.publish(true);
            }
            PresenceCommand::Unregister { client_id } => {
                self.clients.remove(&client_id);
                self.publish(true);
            }
            PresenceCommand::Shutdown => {
                self.publish(true);
                return false;
            }
        }
        true
    }

    fn sample_clients(&mut self) {
        if !self.enabled {
            return;
        }
        for client in self.clients.values_mut() {
            sample_client(client);
        }
        self.publish(false);
    }

    fn publish(&mut self, force: bool) {
        let desired = if self.enabled {
            aggregate_activity(&self.clients)
        } else {
            None
        };
        let now = Instant::now();
        let same_as_sent = desired == self.sent_activity;
        let heartbeat_due = self
            .last_publish
            .is_none_or(|last| now.duration_since(last) >= HEARTBEAT_INTERVAL);
        let retry_due = self
            .last_publish
            .is_none_or(|last| now.duration_since(last) >= RETRY_INTERVAL);
        if !force && same_as_sent && !heartbeat_due && self.last_error.is_none() {
            return;
        }
        if !force && !same_as_sent && self.last_error.is_some() && !retry_due {
            return;
        }
        if !force && same_as_sent && !heartbeat_due && !retry_due {
            return;
        }

        let result = match desired.as_ref() {
            Some(activity) => self.transport.set_activity(activity),
            None if self.sent_activity.is_some() => self.transport.clear_activity(),
            None => Ok(()),
        };
        self.last_publish = Some(now);
        match result {
            Ok(()) => {
                self.sent_activity = desired;
                self.last_error = None;
            }
            Err(error) => {
                if self.last_error.as_deref() != Some(error.as_str()) {
                    eprintln!("[ro-launcher] Discord Rich Presence: {error}");
                }
                self.last_error = Some(error);
            }
        }
    }
}

fn run_worker(receiver: Receiver<PresenceCommand>, transport: Box<dyn PresenceTransport>) {
    let mut worker = PresenceWorker::new(receiver, transport);
    loop {
        match worker.receiver.recv_timeout(SAMPLE_INTERVAL) {
            Ok(command) => {
                if !worker.handle(command) {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                worker.sample_clients();
            }
            Err(RecvTimeoutError::Disconnected) => {
                worker.publish(true);
                break;
            }
        }
    }
}

fn sample_client(client: &mut TrackedClient) {
    let Some(profile) = &client.profile else {
        return;
    };
    if !verify_process_identity(&client.identity) {
        invalidate_client(client);
        return;
    }
    if !address_in_maps(client.identity.pid, profile.module_base) {
        invalidate_client(client);
        return;
    }
    let Ok(memory) = ProcMemoryReader::open(client.identity.pid) else {
        invalidate_client(client);
        return;
    };
    let Ok(snapshot) = read_character_snapshot(&memory, profile) else {
        invalidate_client(client);
        return;
    };
    if !verify_process_identity(&client.identity) {
        invalidate_client(client);
        return;
    }

    if snapshot.state != CharacterState::InGame {
        invalidate_client(client);
        return;
    }

    let key = SnapshotKey::from(&snapshot);
    match client.pending.as_mut() {
        Some(pending) if pending.key == key => {
            pending.samples = pending.samples.saturating_add(1);
            pending.snapshot = snapshot.clone();
        }
        _ => {
            client.pending = Some(PendingSnapshot {
                key,
                snapshot: snapshot.clone(),
                samples: 1,
            });
        }
    }
    let stable = client
        .pending
        .as_ref()
        .is_some_and(|pending| pending.samples >= REQUIRED_STABLE_SAMPLES);
    if stable {
        let snapshot = client
            .pending
            .as_ref()
            .expect("stable pending")
            .snapshot
            .clone();
        if client
            .character_name
            .as_ref()
            .is_some_and(|name| Some(name) != snapshot.character_name.as_ref())
        {
            client.session_started = unix_timestamp();
        }
        client.character_name = snapshot.character_name.clone();
        client.snapshot = Some(snapshot);
        client.invalid_samples = 0;
    }
}

fn invalidate_client(client: &mut TrackedClient) {
    client.pending = None;
    client.invalid_samples = client.invalid_samples.saturating_add(1);
    if client.invalid_samples >= MAX_INVALID_SAMPLES {
        client.snapshot = None;
    }
}

fn aggregate_activity(clients: &HashMap<String, TrackedClient>) -> Option<PresenceActivity> {
    if clients.is_empty() {
        return None;
    }
    if clients.len() == 1 {
        let client = clients.values().next().expect("one client checked");
        return Some(activity_for_client(client));
    }

    let mut servers: Vec<String> = clients
        .values()
        .map(|client| sanitize_public_text(&client.server_name, 128))
        .collect();
    servers.sort_unstable();
    servers.dedup();
    Some(PresenceActivity {
        name: "Ragnarok Online".into(),
        details: format!("{} clientes en juego", clients.len()),
        state: truncate_public_text(&servers.join(" · "), 128),
        start_timestamp: None,
        large_image_key: None,
        large_image_text: None,
    })
}

fn activity_for_client(client: &TrackedClient) -> PresenceActivity {
    let server = sanitize_public_text(&client.server_name, 64);
    let activity_name = if server.is_empty() {
        "Ragnarok Online".to_string()
    } else {
        server.clone()
    };
    let (details, state, start_timestamp) = match &client.snapshot {
        Some(snapshot) if snapshot.state == CharacterState::InGame => {
            let character = snapshot
                .character_name
                .as_deref()
                .map(|name| sanitize_public_text(name, 40));
            let level = level_label(snapshot);
            let map = snapshot
                .map_name
                .as_deref()
                .map(display_map_name)
                .map(|map| sanitize_public_text(&map, 64))
                .unwrap_or_else(|| "Ubicación no disponible".into());
            (
                truncate_public_text(
                    &character_details(character.as_deref(), level.as_deref()),
                    128,
                ),
                map,
                Some(client.session_started),
            )
        }
        _ => (
            "En juego".into(),
            "Ubicación no disponible".into(),
            Some(client.session_started),
        ),
    };

    PresenceActivity {
        name: activity_name,
        details,
        state: truncate_public_text(&state, 128),
        start_timestamp,
        large_image_key: None,
        large_image_text: None,
    }
}

fn level_label(snapshot: &CharacterSnapshot) -> Option<String> {
    match (snapshot.level, snapshot.job_level) {
        (Some(base), Some(job)) => Some(format!("Nv. {base}/{job}")),
        (Some(base), None) => Some(format!("Nv. {base}")),
        (None, Some(job)) => Some(format!("Job {job}")),
        (None, None) => None,
    }
}

fn character_details(character: Option<&str>, level: Option<&str>) -> String {
    match (character, level) {
        (Some(character), Some(level)) => format!("{character} · {level}"),
        (Some(character), None) => character.to_string(),
        (None, Some(level)) => level.to_string(),
        (None, None) => "En juego".into(),
    }
}

impl From<&CharacterSnapshot> for SnapshotKey {
    fn from(snapshot: &CharacterSnapshot) -> Self {
        Self {
            character_name: snapshot.character_name.clone(),
            level: snapshot.level,
            job_level: snapshot.job_level,
            map_name: snapshot.map_name.clone(),
        }
    }
}

fn sanitize_public_text(value: &str, max_len: usize) -> String {
    let sanitized = value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    truncate_public_text(sanitized.trim(), max_len)
}

fn truncate_public_text(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(id: &str, server: &str) -> (String, TrackedClient) {
        (
            id.into(),
            TrackedClient {
                server_name: server.into(),
                identity: ProcessIdentity {
                    pid: 1,
                    start_time: 1,
                },
                profile: None,
                session_started: 123,
                character_name: None,
                snapshot: None,
                pending: None,
                invalid_samples: 0,
            },
        )
    }

    #[test]
    fn single_client_uses_a_safe_fallback_until_memory_is_stable() {
        let (id, client) = client("one", "HoneyRO");
        let activity = aggregate_activity(&HashMap::from([(id, client)])).unwrap();
        assert_eq!(activity.name, "HoneyRO");
        assert_eq!(activity.details, "En juego");
        assert_eq!(activity.state, "Ubicación no disponible");
    }

    #[test]
    fn single_client_uses_server_title_and_compact_base_job_level_details() {
        let (id, mut client) = client("one", "SakuraRO");
        client.character_name = Some("tiny yawn".into());
        client.snapshot = Some(CharacterSnapshot {
            character_name: Some("tiny yawn".into()),
            level: Some(99),
            job_level: Some(70),
            map_name: Some("malaya".into()),
            state: CharacterState::InGame,
            sampled_at: Instant::now(),
        });

        let activity = aggregate_activity(&HashMap::from([(id, client)])).unwrap();
        assert_eq!(activity.name, "SakuraRO");
        assert_eq!(activity.details, "tiny yawn · Nv. 99/70");
        assert_eq!(activity.state, "Port Malaya");
        assert_eq!(activity.start_timestamp, Some(123));
        assert!(activity.large_image_key.is_none());
        assert!(activity.large_image_text.is_none());
    }

    #[test]
    fn multiple_clients_never_select_one_character_arbitrarily() {
        let first = client("one", "HoneyRO");
        let second = client("two", "SakuraRO");
        let activity = aggregate_activity(&HashMap::from([first, second])).unwrap();
        assert_eq!(activity.name, "Ragnarok Online");
        assert_eq!(activity.details, "2 clientes en juego");
        assert_eq!(activity.state, "HoneyRO · SakuraRO");
        assert_eq!(activity.start_timestamp, None);
        assert!(activity.large_image_key.is_none());
    }

    #[test]
    fn empty_server_name_falls_back_to_ragnarok_online() {
        let (id, client) = client("one", "   ");
        let activity = aggregate_activity(&HashMap::from([(id, client)])).unwrap();
        assert_eq!(activity.name, "Ragnarok Online");
    }

    #[test]
    fn public_text_removes_controls_and_truncates_by_characters() {
        assert_eq!(sanitize_public_text("Server\nName", 64), "ServerName");
        assert_eq!(truncate_public_text("áéí", 2), "áé");
    }
}
