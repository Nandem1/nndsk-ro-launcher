use ro_tools_core::{AutobuffConfig, ClientProfile};
use ro_tools_linux::ProcMemoryReader;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio::sync::watch;

use crate::models::autobuff::AutobuffStatusEvent;
use crate::tools::input::{InputGateway, InputSource};
use crate::tools::session::SessionController;
use crate::utils::emit_tool_log_opt;

pub struct AutobuffHandle {
    session: SessionController,
    config_tx: Arc<Mutex<Option<watch::Sender<AutobuffConfig>>>>,
    status: Arc<Mutex<AutobuffStatusEvent>>,
}

impl Clone for AutobuffHandle {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            config_tx: Arc::clone(&self.config_tx),
            status: Arc::clone(&self.status),
        }
    }
}

impl AutobuffHandle {
    pub fn new() -> Self {
        Self {
            session: SessionController::new("AutoBuff"),
            config_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(AutobuffStatusEvent::default())),
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
        self.status.lock().unwrap().active = false;
        result
    }
    pub async fn start(
        &self,
        app: AppHandle,
        pid: u32,
        config: AutobuffConfig,
        profile: ClientProfile,
        input: InputGateway,
    ) -> Result<(), String> {
        let memory = ProcMemoryReader::open(pid)
            .map_err(|e| format!("No se pudo abrir memoria PID {pid}: {e}"))?;
        let config = config.clamped();
        let (config_tx, config_rx) = watch::channel(config.clone());
        let writer = input
            .writer(InputSource::Autobuff, config.delay_ms)
            .map_err(|error| error.to_string())?;
        let status_arc = Arc::clone(&self.status);
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
                    memory,
                    writer,
                    config,
                    profile,
                    stop_rx,
                    config_rx,
                    status_arc,
                })
                .await;
            })
            .await?;
        *self.config_tx.lock().unwrap() = Some(config_tx);
        Ok(())
    }
}
