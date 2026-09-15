use std::sync::{Arc, Mutex};
use std::time::Duration;

use ro_tools_core::{AutopotConfig, ClientProfile, MemoryReader};
use ro_tools_linux::{address_in_maps, ProcessIdentity};
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::time::{interval, Interval, MissedTickBehavior};

use crate::models::autopot::AutopotStatusEvent;
use crate::models::memory::{MemoryAccess, ProfileMemory};
use crate::tools::input::{InputGateway, InputSource};
use crate::tools::memory_sessions::{MemorySession, MemorySessionRegistry, SharedMemorySession};
use crate::tools::session::SessionController;
use crate::utils::emit_tool_log_opt;

use super::scanner::{
    DetectedNameAddress, LevelScanProgress, MapScanProgress, MemoryScanProgress,
    MemoryScannerHandle,
};

pub struct AutopotHandle {
    session: SessionController,
    config_tx: Arc<Mutex<Option<watch::Sender<AutopotConfig>>>>,
    identity_tx: Arc<Mutex<Option<watch::Sender<ProcessIdentity>>>>,
    status: Arc<Mutex<AutopotStatusEvent>>,
    scanner: MemoryScannerHandle,
    memory: MemorySessionRegistry,
}

impl Clone for AutopotHandle {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            config_tx: Arc::clone(&self.config_tx),
            identity_tx: Arc::clone(&self.identity_tx),
            status: Arc::clone(&self.status),
            scanner: self.scanner.clone(),
            memory: self.memory.clone(),
        }
    }
}

impl AutopotHandle {
    pub fn new(memory: MemorySessionRegistry) -> Self {
        Self {
            session: SessionController::new("AutoPot"),
            config_tx: Arc::new(Mutex::new(None)),
            identity_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(AutopotStatusEvent::default())),
            scanner: MemoryScannerHandle::new(memory.clone()),
            memory,
        }
    }

    pub fn status(&self) -> AutopotStatusEvent {
        self.status.lock().unwrap().clone()
    }

    pub fn handoff(&self, identity: ProcessIdentity) {
        if let Some(tx) = self.identity_tx.lock().unwrap().as_ref() {
            let _ = tx.send(identity);
        }
    }

    pub fn update_config(&self, config: AutopotConfig) -> Result<(), String> {
        let guard = self.config_tx.lock().unwrap();
        match guard.as_ref() {
            Some(tx) => tx
                .send(config)
                .map_err(|_| "AutoPot no está activo".to_string()),
            None => Err("AutoPot no está activo".to_string()),
        }
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.scanner.cancel();
        let result = self.session.stop().await;
        *self.config_tx.lock().unwrap() = None;
        *self.identity_tx.lock().unwrap() = None;
        let mut status = self.status.lock().unwrap();
        status.active = false;
        result
    }

    pub async fn begin_memory_scan(
        &self,
        identity: ProcessIdentity,
        current_hp: u32,
    ) -> Result<MemoryScanProgress, String> {
        self.scanner.begin(&self.memory, identity, current_hp).await
    }

    pub async fn refine_memory_scan(&self, current_hp: u32) -> Result<MemoryScanProgress, String> {
        self.scanner.refine(&self.memory, current_hp).await
    }

    pub fn cancel_memory_scan(&self) {
        self.scanner.cancel();
    }

    pub async fn find_name_address(
        &self,
        identity: ProcessIdentity,
        character_name: String,
        hp_base: Option<u32>,
    ) -> Result<DetectedNameAddress, String> {
        self.scanner
            .find_name(&self.memory, identity, character_name, hp_base)
            .await
    }

    pub async fn begin_level_scan(
        &self,
        identity: ProcessIdentity,
        current_level: u32,
        name_address: Option<u32>,
        hp_base: Option<u32>,
    ) -> Result<LevelScanProgress, String> {
        self.scanner
            .begin_level(&self.memory, identity, current_level, name_address, hp_base)
            .await
    }

    pub async fn refine_level_scan(&self, current_level: u32) -> Result<LevelScanProgress, String> {
        self.scanner.refine_level(&self.memory, current_level).await
    }

    pub async fn begin_map_scan(
        &self,
        identity: ProcessIdentity,
        map_name: String,
        name_address: Option<u32>,
        hp_base: Option<u32>,
        level_address: Option<u32>,
    ) -> Result<MapScanProgress, String> {
        self.scanner
            .begin_map(
                &self.memory,
                identity,
                map_name,
                name_address,
                hp_base,
                level_address,
            )
            .await
    }

    pub async fn refine_map_scan(&self, map_name: String) -> Result<MapScanProgress, String> {
        self.scanner.refine_map(&self.memory, map_name).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        app: AppHandle,
        identity: ProcessIdentity,
        memory: Arc<MemorySession>,
        config: AutopotConfig,
        profile: ClientProfile,
        input: InputGateway,
        memory_access: MemoryAccess,
        profile_memory: ProfileMemory,
    ) -> Result<(), String> {
        self.scanner.cancel();
        log_startup_probe(&app, identity.pid, &memory, &profile);

        let config = config.clamped();
        let writer = input
            .writer(InputSource::Autopot, config.delay_ms)
            .map_err(|error| error.to_string())?;

        let (config_tx, config_rx) = watch::channel(config.clone());
        let (identity_tx, identity_rx) = watch::channel(identity);
        let status_arc = Arc::clone(&self.status);
        let memory_registry = self.memory.clone();
        let hp_base = profile.hp_base;

        emit_tool_log_opt(
            Some(&app),
            format!(
                "[AutoPot] Loop iniciado backend=uinput delay efectivo={}ms",
                config.delay_ms,
            ),
        );

        self.session
            .replace(move |stop_rx| async move {
                super::loop_runner::run(super::loop_runner::RunContext {
                    app,
                    memory: SharedMemorySession(memory),
                    writer,
                    config,
                    profile,
                    stop_rx,
                    config_rx,
                    identity_rx,
                    status_arc,
                    gateway: input,
                    memory_registry,
                    hp_base,
                    memory_access,
                    profile_memory,
                })
                .await;
            })
            .await?;
        *self.config_tx.lock().unwrap() = Some(config_tx);
        *self.identity_tx.lock().unwrap() = Some(identity_tx);
        Ok(())
    }
}

fn log_startup_probe(app: &AppHandle, pid: u32, memory: &MemorySession, profile: &ClientProfile) {
    let mapped = address_in_maps(pid, profile.hp_base);
    emit_tool_log_opt(
        Some(app),
        format!(
            "[AutoPot] Memoria: PID={pid} HP addr {:#x} mapped={mapped}",
            profile.hp_base
        ),
    );

    match memory.probe_stats(profile.hp_base) {
        Ok(_) => emit_tool_log_opt(Some(app), "[AutoPot] Probe OK"),
        Err(e) => emit_tool_log_opt(Some(app), format!("[AutoPot] Probe falló: {e}")),
    }

    match memory.read_string(profile.name_address, 40) {
        Ok(name) if !name.is_empty() => {
            emit_tool_log_opt(Some(app), "[AutoPot] Personaje detectado en memoria");
        }
        Ok(_) => emit_tool_log_opt(Some(app), "[AutoPot] Nombre vacío (¿en char select?)"),
        Err(e) => emit_tool_log_opt(Some(app), format!("[AutoPot] Nombre no leído: {e}")),
    }
}

pub(crate) fn new_ticker(delay_ms: u64) -> Interval {
    let mut ticker = interval(Duration::from_millis(delay_ms.max(10)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}
