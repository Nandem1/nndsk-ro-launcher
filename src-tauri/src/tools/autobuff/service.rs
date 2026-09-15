use ro_tools_core::{AutobuffConfig, ClientProfile};
use ro_tools_linux::ProcessIdentity;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio::sync::watch;

use crate::models::autobuff::AutobuffStatusEvent;
use crate::models::memory::{MemoryAccess, ProfileMemory};
use crate::tools::input::{InputGateway, InputSource};
use crate::tools::memory_sessions::{MemorySession, MemorySessionRegistry, SharedMemorySession};
use crate::tools::session::SessionController;
use crate::utils::emit_tool_log_opt;

pub struct AutobuffHandle {
    session: SessionController,
    config_tx: Arc<Mutex<Option<watch::Sender<AutobuffConfig>>>>,
    identity_tx: Arc<Mutex<Option<watch::Sender<ProcessIdentity>>>>,
    status: Arc<Mutex<AutobuffStatusEvent>>,
    memory: MemorySessionRegistry,
}

impl Clone for AutobuffHandle {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            config_tx: Arc::clone(&self.config_tx),
            identity_tx: Arc::clone(&self.identity_tx),
            status: Arc::clone(&self.status),
            memory: self.memory.clone(),
        }
    }
}

impl AutobuffHandle {
    pub fn new(memory: MemorySessionRegistry) -> Self {
        Self {
            session: SessionController::new("AutoBuff"),
            config_tx: Arc::new(Mutex::new(None)),
            identity_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(AutobuffStatusEvent::default())),
            memory,
        }
    }

    pub fn handoff(&self, identity: ProcessIdentity) {
        if let Some(tx) = self.identity_tx.lock().unwrap().as_ref() {
            let _ = tx.send(identity);
        }
    }

    pub fn status(&self) -> AutobuffStatusEvent {
        self.status.lock().unwrap().clone()
    }

    pub fn update_config(&self, config: AutobuffConfig) -> Result<(), String> {
        self.config_tx
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| "AutoBuff no está activo".to_string())?
            .send(config.clamped())
            .map_err(|_| "AutoBuff no está activo".to_string())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let result = self.session.stop().await;
        *self.config_tx.lock().unwrap() = None;
        *self.identity_tx.lock().unwrap() = None;
        self.status.lock().unwrap().active = false;
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        app: AppHandle,
        identity: ProcessIdentity,
        memory: Arc<MemorySession>,
        config: AutobuffConfig,
        profile: ClientProfile,
        input: InputGateway,
        memory_access: MemoryAccess,
        profile_memory: ProfileMemory,
    ) -> Result<(), String> {
        let config = config.clamped();
        let (config_tx, config_rx) = watch::channel(config.clone());
        let (identity_tx, identity_rx) = watch::channel(identity);
        let writer = input
            .writer(InputSource::Autobuff, config.delay_ms)
            .map_err(|error| error.to_string())?;
        let status_arc = Arc::clone(&self.status);
        let memory_registry = self.memory.clone();
        let hp_base = profile.hp_base;
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[AutoBuff] Loop iniciado | buffer={:#x} | {} reglas",
                profile.status_buffer_address(),
                config.rules.len()
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
