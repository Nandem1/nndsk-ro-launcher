use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ro_tools_core::{
    read_character_snapshot, resolve_presence_memory_profiles, CharacterSnapshot, CharacterState,
    PresenceAddressOverrides, PresenceMemoryProfile,
};
use ro_tools_linux::{address_in_maps, verify_process_identity, ProcessIdentity};

use crate::tools::memory_sessions::MemorySessionRegistry;

use super::map_names::display_map_name;
use super::profiles::resolve_runtime_profiles;
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
    pub fn new(memory: MemorySessionRegistry) -> Self {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("ro-presence".into())
            .spawn(move || run_worker(receiver, default_transport(), memory))
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
        server_id: String,
        server_name: String,
        identity: ProcessIdentity,
        executable_path: String,
        overrides: PresenceAddressOverrides,
    ) {
        let _ = self.commands.send(PresenceCommand::Register {
            client_id,
            server_id,
            server_name,
            identity,
            executable_path,
            overrides,
        });
    }

    pub fn apply_overrides(&self, server_id: &str, overrides: PresenceAddressOverrides) {
        let _ = self.commands.send(PresenceCommand::ApplyOverrides {
            server_id: server_id.to_string(),
            overrides,
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
        Self::new(MemorySessionRegistry::new())
    }
}

enum PresenceCommand {
    SetEnabled(bool),
    Register {
        client_id: String,
        server_id: String,
        server_name: String,
        identity: ProcessIdentity,
        executable_path: String,
        overrides: PresenceAddressOverrides,
    },
    ApplyOverrides {
        server_id: String,
        overrides: PresenceAddressOverrides,
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
    memory: MemorySessionRegistry,
    enabled: bool,
    clients: HashMap<String, TrackedClient>,
    sent_activity: Option<PresenceActivity>,
    last_publish: Option<Instant>,
    last_error: Option<String>,
}

struct TrackedClient {
    server_id: String,
    server_name: String,
    identity: ProcessIdentity,
    from_hash: bool,
    applied_overrides: PresenceAddressOverrides,
    profile: Option<PresenceMemoryProfile>,
    derived_candidates: Vec<PresenceMemoryProfile>,
    session_started: i64,
    character_name: Option<String>,
    snapshot: Option<CharacterSnapshot>,
    pending: Option<PendingSnapshot>,
    invalid_samples: u8,
}

struct PendingSnapshot {
    key: SnapshotKey,
    profile: PresenceMemoryProfile,
    snapshot: CharacterSnapshot,
    samples: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotKey {
    profile_id: String,
    character_name: Option<String>,
    level: Option<u32>,
    job_level: Option<u32>,
    map_name: Option<String>,
}

impl PresenceWorker {
    fn new(
        receiver: Receiver<PresenceCommand>,
        transport: Box<dyn PresenceTransport>,
        memory: MemorySessionRegistry,
    ) -> Self {
        Self {
            receiver,
            transport,
            memory,
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
                server_id,
                server_name,
                identity,
                executable_path,
                overrides,
            } => {
                let (profile, derived_candidates) =
                    resolve_runtime_profiles(&executable_path, overrides);
                self.clients.insert(
                    client_id,
                    TrackedClient {
                        server_id,
                        server_name,
                        identity,
                        from_hash: profile.is_some(),
                        applied_overrides: overrides,
                        profile,
                        derived_candidates,
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
            PresenceCommand::ApplyOverrides {
                server_id,
                overrides,
            } => {
                let mut changed = false;
                for client in self.clients.values_mut() {
                    if client.server_id != server_id {
                        continue;
                    }
                    changed |= apply_overrides_to_client(client, overrides);
                }
                if changed {
                    self.sample_clients();
                    self.publish(true);
                }
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
            sample_client(client, &self.memory);
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

fn run_worker(
    receiver: Receiver<PresenceCommand>,
    transport: Box<dyn PresenceTransport>,
    memory: MemorySessionRegistry,
) {
    let mut worker = PresenceWorker::new(receiver, transport, memory);
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

fn apply_overrides_to_client(
    client: &mut TrackedClient,
    overrides: PresenceAddressOverrides,
) -> bool {
    if client.from_hash {
        return false;
    }
    if client.applied_overrides == overrides {
        return false;
    }
    client.applied_overrides = overrides;
    let (profile, derived_candidates) = resolve_presence_memory_profiles(None, overrides);
    client.profile = profile;
    client.derived_candidates = derived_candidates;
    client.snapshot = None;
    client.pending = None;
    client.invalid_samples = 0;
    true
}

fn sample_client(client: &mut TrackedClient, memory: &MemorySessionRegistry) {
    if client.profile.is_none() && client.derived_candidates.is_empty() {
        return;
    }
    if !verify_process_identity(&client.identity) {
        invalidate_client(client);
        return;
    }

    let profiles = match &client.profile {
        Some(profile) => vec![profile.clone()],
        None => client.derived_candidates.clone(),
    };
    let Some((profile, snapshot)) = read_ingame_candidate(&client.identity, memory, &profiles)
    else {
        invalidate_client(client);
        return;
    };
    if !verify_process_identity(&client.identity) {
        invalidate_client(client);
        return;
    }
    record_ingame_sample(client, profile, snapshot);
}

fn read_ingame_candidate(
    identity: &ProcessIdentity,
    memory: &MemorySessionRegistry,
    profiles: &[PresenceMemoryProfile],
) -> Option<(PresenceMemoryProfile, CharacterSnapshot)> {
    let session = memory.get(identity)?;
    if !session.access().usable() {
        return None;
    }
    for profile in profiles {
        if !address_in_maps(identity.pid, profile.module_base) {
            continue;
        }
        let Ok(snapshot) = read_character_snapshot(session.as_ref(), profile) else {
            continue;
        };
        if snapshot.state == CharacterState::InGame {
            return Some((profile.clone(), snapshot));
        }
    }
    None
}

fn record_ingame_sample(
    client: &mut TrackedClient,
    profile: PresenceMemoryProfile,
    snapshot: CharacterSnapshot,
) {
    let key = SnapshotKey::from_sample(&profile.id, &snapshot);
    match client.pending.as_mut() {
        Some(pending) if pending.key == key => {
            pending.samples = pending.samples.saturating_add(1);
            pending.snapshot = snapshot;
            pending.profile = profile;
        }
        _ => {
            client.pending = Some(PendingSnapshot {
                key,
                profile,
                snapshot,
                samples: 1,
            });
        }
    }
    let stable = client
        .pending
        .as_ref()
        .is_some_and(|pending| pending.samples >= REQUIRED_STABLE_SAMPLES);
    if !stable {
        return;
    }
    let pending = client.pending.as_ref().expect("stable pending");
    if !client.from_hash {
        client.profile = Some(pending.profile.clone());
    }
    let snapshot = pending.snapshot.clone();
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

fn invalidate_client(client: &mut TrackedClient) {
    client.pending = None;
    client.invalid_samples = client.invalid_samples.saturating_add(1);
    if client.invalid_samples >= MAX_INVALID_SAMPLES {
        client.snapshot = None;
        if !client.from_hash {
            client.profile = None;
        }
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

impl SnapshotKey {
    fn from_sample(profile_id: &str, snapshot: &CharacterSnapshot) -> Self {
        Self {
            profile_id: profile_id.to_string(),
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
                server_id: id.into(),
                server_name: server.into(),
                identity: ProcessIdentity {
                    pid: 1,
                    start_time: 1,
                },
                from_hash: false,
                applied_overrides: PresenceAddressOverrides::default(),
                profile: None,
                derived_candidates: Vec::new(),
                session_started: 123,
                character_name: None,
                snapshot: None,
                pending: None,
                invalid_samples: 0,
            },
        )
    }

    fn memory_profile(id: &str) -> PresenceMemoryProfile {
        PresenceMemoryProfile {
            id: id.into(),
            exe_names: Vec::new(),
            executable_sha256: String::new(),
            pe_build_timestamp: String::new(),
            image_size: 0,
            module_base: 0x0040_0000,
            name_address: 0x1000,
            level_address: 0x1004,
            job_level_address: 0x1008,
            map_address: 0x100c,
        }
    }

    fn ingame_snapshot() -> CharacterSnapshot {
        CharacterSnapshot {
            character_name: Some("Hero".into()),
            level: Some(10),
            job_level: Some(7),
            map_name: Some("prontera".into()),
            state: CharacterState::InGame,
            sampled_at: Instant::now(),
        }
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

    #[test]
    fn apply_with_the_same_anchors_does_not_clear_snapshot() {
        let (_, mut client) = client("one", "HoneyRO");
        client.applied_overrides = PresenceAddressOverrides {
            name: Some(0x010D_F5D8),
            hp: Some(0x010D_CE10),
            ..PresenceAddressOverrides::default()
        };
        client.snapshot = Some(ingame_snapshot());
        client.character_name = Some("Hero".into());

        assert!(!apply_overrides_to_client(
            &mut client,
            PresenceAddressOverrides {
                name: Some(0x010D_F5D8),
                hp: Some(0x010D_CE10),
                ..PresenceAddressOverrides::default()
            }
        ));
        assert!(client.snapshot.is_some());
        assert_eq!(client.character_name.as_deref(), Some("Hero"));
    }

    #[test]
    fn hash_client_is_not_reset_when_overrides_change() {
        let (_, mut client) = client("one", "HoneyRO");
        client.from_hash = true;
        client.profile = Some(memory_profile("honey-ro-ragexe-2018-06-21"));
        client.snapshot = Some(ingame_snapshot());

        assert!(!apply_overrides_to_client(
            &mut client,
            PresenceAddressOverrides {
                name: Some(0x010D_F5D8),
                ..PresenceAddressOverrides::default()
            }
        ));
        assert!(client.snapshot.is_some());
        assert_eq!(
            client.profile.as_ref().map(|profile| profile.id.as_str()),
            Some("honey-ro-ragexe-2018-06-21")
        );
    }

    #[test]
    fn first_ingame_sample_does_not_lock_a_derived_family() {
        let (_, mut client) = client("one", "CustomRO");
        let rathena = memory_profile("derived-rathena-2018");
        client.derived_candidates = vec![rathena.clone(), memory_profile("derived-sakura-2025")];

        record_ingame_sample(&mut client, rathena, ingame_snapshot());

        assert!(client.profile.is_none());
        assert!(client.snapshot.is_none());
        assert_eq!(
            client.pending.as_ref().map(|pending| pending.samples),
            Some(1)
        );
    }

    #[test]
    fn second_matching_sample_locks_the_derived_family() {
        let (_, mut client) = client("one", "CustomRO");
        let rathena = memory_profile("derived-rathena-2018");

        record_ingame_sample(&mut client, rathena.clone(), ingame_snapshot());
        record_ingame_sample(&mut client, rathena, ingame_snapshot());

        assert_eq!(
            client.profile.as_ref().map(|profile| profile.id.as_str()),
            Some("derived-rathena-2018")
        );
        assert_eq!(
            client
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.map_name.as_deref()),
            Some("prontera")
        );
    }

    #[test]
    fn switching_family_resets_stability_before_lock() {
        let (_, mut client) = client("one", "CustomRO");

        record_ingame_sample(
            &mut client,
            memory_profile("derived-rathena-2018"),
            ingame_snapshot(),
        );
        record_ingame_sample(
            &mut client,
            memory_profile("derived-sakura-2025"),
            ingame_snapshot(),
        );

        assert!(client.profile.is_none());
        assert_eq!(
            client
                .pending
                .as_ref()
                .map(|pending| pending.key.profile_id.as_str()),
            Some("derived-sakura-2025")
        );
        assert_eq!(
            client.pending.as_ref().map(|pending| pending.samples),
            Some(1)
        );
    }

    #[test]
    fn derived_invalid_samples_unlock_the_profile() {
        let (_, mut client) = client("one", "CustomRO");
        client.profile = Some(memory_profile("derived-rathena-2018"));
        client.snapshot = Some(ingame_snapshot());
        client.derived_candidates = vec![memory_profile("derived-rathena-2018")];

        for _ in 0..MAX_INVALID_SAMPLES {
            invalidate_client(&mut client);
        }

        assert!(client.profile.is_none());
        assert!(client.snapshot.is_none());
    }

    #[test]
    fn hash_profile_stays_locked_after_invalid_samples() {
        let (_, mut client) = client("one", "HoneyRO");
        client.from_hash = true;
        client.profile = Some(memory_profile("honey-ro-ragexe-2018-06-21"));
        client.snapshot = Some(ingame_snapshot());

        for _ in 0..MAX_INVALID_SAMPLES {
            invalidate_client(&mut client);
        }

        assert_eq!(
            client.profile.as_ref().map(|profile| profile.id.as_str()),
            Some("honey-ro-ragexe-2018-06-21")
        );
        assert!(client.snapshot.is_none());
    }

    #[test]
    fn apply_new_name_override_rederives_candidates() {
        let (_, mut client) = client("one", "CustomRO");
        client.snapshot = Some(ingame_snapshot());

        assert!(apply_overrides_to_client(
            &mut client,
            PresenceAddressOverrides {
                name: Some(0x010D_F5D8),
                hp: Some(0x010D_CE10),
                ..PresenceAddressOverrides::default()
            }
        ));
        assert!(client.snapshot.is_none());
        assert!(client.profile.is_none());
        assert_eq!(client.derived_candidates.len(), 1);
        assert_eq!(client.derived_candidates[0].map_address, 0x010D_856C);
    }

    #[test]
    fn apply_explicit_level_and_map_builds_a_scanned_profile() {
        let (_, mut client) = client("one", "KlaipedaRO");

        assert!(apply_overrides_to_client(
            &mut client,
            PresenceAddressOverrides {
                name: Some(0x0177_AE00),
                hp: Some(0x0177_8190),
                level: Some(0x0177_8000),
                job: Some(0x0177_8008),
                map: Some(0x0177_7000),
            }
        ));
        assert!(client.profile.is_none());
        assert_eq!(client.derived_candidates.len(), 1);
        assert_eq!(client.derived_candidates[0].id, "scanned");
        assert_eq!(client.derived_candidates[0].map_address, 0x0177_7000);
    }
}
