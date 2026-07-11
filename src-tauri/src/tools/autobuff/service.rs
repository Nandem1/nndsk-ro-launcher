use std::sync::{Arc, Mutex};
use std::time::Duration;

use ro_tools_core::{AutobuffConfig, ClientProfile};
use ro_tools_linux::ProcMemoryReader;
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::time::sleep;

use crate::models::autobuff::AutobuffStatusEvent;
use crate::tools::input::{InputGateway, YdotoolDaemon};
use crate::utils::emit_tool_log_opt;

pub struct AutobuffHandle {
    stop_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    config_tx: Arc<Mutex<Option<watch::Sender<AutobuffConfig>>>>,
    status: Arc<Mutex<AutobuffStatusEvent>>,
}

impl Clone for AutobuffHandle {
    fn clone(&self) -> Self {
        Self {
            stop_tx: Arc::clone(&self.stop_tx),
            config_tx: Arc::clone(&self.config_tx),
            status: Arc::clone(&self.status),
        }
    }
}

impl AutobuffHandle {
    pub fn new() -> Self {
        Self {
            stop_tx: Arc::new(Mutex::new(None)),
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
    pub async fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(true);
        }
        *self.config_tx.lock().unwrap() = None;
        self.status.lock().unwrap().active = false;
    }
    pub async fn start(
        &self,
        app: AppHandle,
        pid: u32,
        config: AutobuffConfig,
        profile: ClientProfile,
        input: InputGateway,
        ydotoold: Arc<YdotoolDaemon>,
    ) -> Result<(), String> {
        self.stop().await;
        let memory = ProcMemoryReader::open(pid)
            .map_err(|e| format!("No se pudo abrir memoria PID {pid}: {e}"))?;
        let config = config.clamped();
        let (stop_tx, stop_rx) = watch::channel(false);
        let (config_tx, config_rx) = watch::channel(config.clone());
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.config_tx.lock().unwrap() = Some(config_tx);
        emit_tool_log_opt(
            Some(&app),
            format!(
                "[AutoBuff] Loop iniciado | buffer={:#x} | {} reglas",
                profile.status_buffer_address(),
                config.rules.len()
            ),
        );
        tokio::spawn(super::loop_runner::run(
            app,
            memory,
            input.writer(),
            config,
            profile,
            stop_rx,
            config_rx,
            Arc::clone(&self.status),
            input,
            ydotoold,
        ));
        sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}
