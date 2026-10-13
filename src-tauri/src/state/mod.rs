mod game_process;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;

pub(crate) use game_process::GameLaunchAdmission;
pub use game_process::{GameProcessHandle, LaunchReservation};

use crate::tools::autobuff::AutobuffHandle;
use crate::tools::autopot::AutopotHandle;
use crate::tools::input::InputGateway;
use crate::tools::memory_sessions::MemorySessionRegistry;
use crate::tools::presence::PresenceHandle;
use crate::tools::runner_sessions::RunnerSessionRegistry;
use crate::tools::runners::managed_proton_path;
use crate::tools::spammer::SpammerHandle;
use crate::{
    models::{
        server::{validate_servers, ServerConfig},
        settings::AppSettings,
        storage::{StorageNotice, StorageNoticeKind, StorageNoticeSource},
    },
    utils::{
        load_json_recovering, load_settings_document, save_settings_document, servers_path,
        JsonLoadStatus, SettingsDocumentOrigin,
    },
};

/// Estado compartido de la app entre comandos Tauri (juego, tools, input).
pub struct GameState {
    pub game: GameProcessHandle,
    pub tool_lifecycle: Arc<AsyncMutex<()>>,
    pub autopot: AutopotHandle,
    pub autobuff: AutobuffHandle,
    pub spammer: SpammerHandle,
    pub input: InputGateway,
    pub presence: PresenceHandle,
    pub sessions: RunnerSessionRegistry,
    pub memory: MemorySessionRegistry,
    pub installations: crate::tools::installations::InstallationManager,
}

pub struct ServerRepository {
    lock: Mutex<()>,
    path: PathBuf,
}

impl Default for ServerRepository {
    fn default() -> Self {
        Self {
            lock: Mutex::new(()),
            path: servers_path(),
        }
    }
}

impl ServerRepository {
    pub fn insert_installed(
        &self,
        server: ServerConfig,
        notices: &StorageNotices,
    ) -> Result<ServerConfig, String> {
        if server.installation.is_none() {
            return Err("El servidor no tiene una instalación verificada".into());
        }
        server.validate_executable_available()?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "El repositorio de servidores está bloqueado")?;
        let loaded = load_json_recovering(&self.path, |servers: &Vec<ServerConfig>| {
            validate_servers(servers)
        })?;
        notices.record_status(
            loaded.status,
            StorageNoticeSource::Servers,
            "La configuración de servidores fue migrada al formato actual",
            "Se recuperaron los servidores desde el backup; el archivo dañado fue preservado",
        )?;
        let mut servers = loaded.value;
        if let Some(existing) = servers.iter().find(|existing| existing.id == server.id) {
            if existing.installation == server.installation {
                return Ok(existing.clone());
            }
            return Err(
                "El identificador ya pertenece a otro servidor; se conservó su configuración"
                    .into(),
            );
        }
        servers.push(server.clone());
        crate::utils::write_json(&self.path, &servers)?;
        Ok(server)
    }

    pub fn list(&self, notices: &StorageNotices) -> Result<Vec<ServerConfig>, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "El repositorio de servidores está bloqueado".to_string())?;
        let loaded = load_json_recovering(&self.path, |servers: &Vec<ServerConfig>| {
            validate_servers(servers)
        })?;
        notices.record_status(
            loaded.status,
            StorageNoticeSource::Servers,
            "La configuración de servidores fue migrada al formato actual",
            "Se recuperaron los servidores desde el backup; el archivo dañado fue preservado",
        )?;
        Ok(loaded.value)
    }

    pub fn save(&self, servers: &[ServerConfig]) -> Result<(), String> {
        crate::models::server::validate_servers(servers)?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "El repositorio de servidores está bloqueado".to_string())?;
        crate::utils::write_json(&self.path, servers)
    }
}

pub struct SettingsRepository;

impl SettingsRepository {
    pub fn load(&self, notices: &StorageNotices) -> Result<AppSettings, String> {
        let loaded = load_settings_document()?;
        notices.record_status(
            loaded.status,
            StorageNoticeSource::Settings,
            "La configuración general fue migrada al formato actual",
            "Se recuperó la configuración general desde el backup; el archivo dañado fue preservado",
        )?;
        Ok(apply_product_default(loaded.value, loaded.origin))
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        save_settings_document(settings)
    }
}

fn apply_product_default(mut settings: AppSettings, origin: SettingsDocumentOrigin) -> AppSettings {
    if origin == SettingsDocumentOrigin::Missing {
        settings.default_runner = managed_proton_path().to_string_lossy().into_owned();
    }
    settings
}

#[derive(Default)]
pub struct StorageNotices {
    inner: Mutex<Vec<StorageNotice>>,
}

impl StorageNotices {
    pub fn push(&self, notice: StorageNotice) -> Result<(), String> {
        self.inner
            .lock()
            .map_err(|_| "Los avisos de almacenamiento están bloqueados".to_string())?
            .push(notice);
        Ok(())
    }

    pub fn take(&self) -> Result<Vec<StorageNotice>, String> {
        let mut notices = self
            .inner
            .lock()
            .map_err(|_| "Los avisos de almacenamiento están bloqueados".to_string())?;
        Ok(std::mem::take(&mut *notices))
    }

    fn record_status(
        &self,
        status: JsonLoadStatus,
        source: StorageNoticeSource,
        migrated: &str,
        recovered: &str,
    ) -> Result<(), String> {
        let (kind, message) = match status {
            JsonLoadStatus::Unchanged => return Ok(()),
            JsonLoadStatus::Migrated => (StorageNoticeKind::Migrated, migrated),
            JsonLoadStatus::Recovered => (StorageNoticeKind::Recovered, recovered),
        };
        self.push(StorageNotice {
            source,
            kind,
            message: message.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{backup_path, load_json_recovering, JsonLoadStatus};
    use serde_json::json;
    use std::{
        fs,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
        thread,
    };

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_path(name: &str) -> PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "ro-launcher-repository-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory.join("servers.json")
    }

    fn repository(path: PathBuf) -> ServerRepository {
        ServerRepository {
            lock: Mutex::new(()),
            path,
        }
    }

    fn server(id: &str) -> ServerConfig {
        serde_json::from_value(json!({
            "id": id,
            "name": format!("Server {id}"),
            "executablePath": "/games/test/Ragexe.exe"
        }))
        .unwrap()
    }

    #[test]
    fn migrates_legacy_servers_and_records_one_notice() {
        let path = test_path("legacy");
        let legacy = json!([{
            "id": "legacy",
            "name": "Legacy RO",
            "executablePath": "/games/legacy/Ragexe.exe",
            "combatInputBackend": "lowLatency",
            "spammer": {
                "keys": ["F3", "F4"],
                "gearSwitch": {
                    "enabled": true,
                    "triggerKeys": [],
                    "atkKeys": ["8"],
                    "defKeys": ["9"]
                }
            }
        }]);
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let repository = repository(path.clone());
        let notices = StorageNotices::default();

        let loaded = repository.list(&notices).unwrap();
        assert_eq!(loaded[0].spammer.gear_switch.rules.len(), 2);
        let canonical: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(canonical[0]["spammer"]["gearSwitch"]
            .get("triggerKeys")
            .is_none());
        assert_eq!(
            canonical[0]["spammer"]["gearSwitch"]["rules"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(canonical[0].get("combatInputBackend").is_none());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &fs::read_to_string(backup_path(&path)).unwrap()
            )
            .unwrap(),
            legacy
        );
        let drained = notices.take().unwrap();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].kind, StorageNoticeKind::Migrated);
        assert!(notices.take().unwrap().is_empty());
    }

    #[test]
    fn installation_confirmation_is_idempotent_and_preserves_existing_edits() {
        let path = test_path("installation");
        let root = path.parent().unwrap();
        let exe = root.join("client.exe");
        fs::write(&exe, b"fixture").unwrap();
        let mut installed = server("installed-test");
        installed.executable_path = exe.to_string_lossy().into();
        installed.runner = Some("/managed/proton".into());
        installed.installation = Some(crate::models::installation::InstallationBinding {
            installation_id: uuid::Uuid::new_v4().to_string(),
            prefix_path: root.join("prefix").to_string_lossy().into(),
            prefix_fingerprint: "a".repeat(64),
        });
        let repository = repository(path.clone());
        let notices = StorageNotices::default();
        repository
            .insert_installed(installed.clone(), &notices)
            .unwrap();
        let mut edited = installed.clone();
        edited.name = "User edit".into();
        repository.save(&[edited]).unwrap();
        assert_eq!(
            repository
                .insert_installed(installed.clone(), &notices)
                .unwrap()
                .name,
            "User edit"
        );
        assert_eq!(repository.list(&notices).unwrap().len(), 1);
        installed.installation.as_mut().unwrap().installation_id = uuid::Uuid::new_v4().to_string();
        assert!(repository.insert_installed(installed, &notices).is_err());
        assert_eq!(repository.list(&notices).unwrap()[0].name, "User edit");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serializes_concurrent_server_writes() {
        let path = test_path("concurrent");
        let repository = Arc::new(repository(path.clone()));
        let handles: Vec<_> = (0..6)
            .map(|index| {
                let repository = Arc::clone(&repository);
                thread::spawn(move || repository.save(&[server(&index.to_string())]))
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }

        let stored: Vec<ServerConfig> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored.len(), 1);
        stored[0].validate().unwrap();
    }

    #[test]
    fn recovers_settings_from_backup_with_the_same_storage_contract() {
        let path = test_path("settings").with_file_name("settings.json");
        fs::write(&path, "broken").unwrap();
        fs::write(backup_path(&path), r#"{"defaultRunner":"/usr/bin/wine"}"#).unwrap();

        let loaded = load_json_recovering::<AppSettings, _>(&path, AppSettings::validate).unwrap();
        assert_eq!(loaded.status, JsonLoadStatus::Recovered);
        assert_eq!(loaded.value.default_runner, "/usr/bin/wine");
        assert!(!loaded.value.rich_presence_enabled);
        assert_eq!(
            fs::read_to_string(backup_path(&path)).unwrap(),
            r#"{"defaultRunner":"/usr/bin/wine"}"#
        );
    }

    #[test]
    fn product_default_applies_only_when_the_settings_document_is_missing() {
        let managed = managed_proton_path().to_string_lossy().into_owned();
        let fallback = AppSettings::default();
        assert_eq!(
            apply_product_default(fallback.clone(), SettingsDocumentOrigin::Missing).default_runner,
            managed
        );

        for (origin, runner) in [
            (SettingsDocumentOrigin::Persisted, "/usr/bin/wine"),
            (
                SettingsDocumentOrigin::Persisted,
                "/opt/portable-wine/bin/wine",
            ),
            (
                SettingsDocumentOrigin::Persisted,
                "/opt/external-proton/proton",
            ),
            (SettingsDocumentOrigin::Recovered, "/backup/bin/wine"),
        ] {
            let settings = AppSettings {
                default_runner: runner.to_string(),
                rich_presence_enabled: false,
            };
            assert_eq!(
                apply_product_default(settings, origin).default_runner,
                runner
            );
        }
    }
}
