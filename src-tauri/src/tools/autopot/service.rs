use std::sync::{Arc, Mutex};
use std::time::Duration;

use ro_tools_core::{AutopotConfig, ClientProfile, MemoryReader};
use ro_tools_linux::{address_in_maps, ProcMemoryReader};
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::time::{interval, Interval, MissedTickBehavior};

use crate::models::autopot::AutopotStatusEvent;
use crate::tools::input::{InputGateway, InputSource};
use crate::tools::session::SessionController;
use crate::utils::emit_tool_log_opt;

pub struct AutopotHandle {
    session: SessionController,
    config_tx: Arc<Mutex<Option<watch::Sender<AutopotConfig>>>>,
    status: Arc<Mutex<AutopotStatusEvent>>,
}

impl Clone for AutopotHandle {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            config_tx: Arc::clone(&self.config_tx),
            status: Arc::clone(&self.status),
        }
    }
}

impl AutopotHandle {
    pub fn new() -> Self {
        Self {
            session: SessionController::new("AutoPot"),
            config_tx: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(AutopotStatusEvent::default())),
        }
    }

    pub fn status(&self) -> AutopotStatusEvent {
        self.status.lock().unwrap().clone()
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
        let result = self.session.stop().await;
        *self.config_tx.lock().unwrap() = None;
        let mut status = self.status.lock().unwrap();
        status.active = false;
        result
    }

    pub async fn start(
        &self,
        app: AppHandle,
        pid: u32,
        config: AutopotConfig,
        profile: ClientProfile,
        input: InputGateway,
    ) -> Result<(), String> {
        let memory = ProcMemoryReader::open(pid)
            .map_err(|e| format!("No se pudo abrir memoria PID {pid}: {e}"))?;

        log_startup_probe(&app, pid, &memory, &profile);

        let config = config.clamped();
        let writer = input
            .writer(InputSource::Autopot, config.delay_ms)
            .map_err(|error| error.to_string())?;

        let (config_tx, config_rx) = watch::channel(config.clone());
        let status_arc = Arc::clone(&self.status);

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
                    memory,
                    writer,
                    config,
                    profile,
                    stop_rx,
                    config_rx,
                    status_arc,
                    gateway: input,
                })
                .await;
            })
            .await?;
        *self.config_tx.lock().unwrap() = Some(config_tx);
        Ok(())
    }
}

fn log_startup_probe(
    app: &AppHandle,
    pid: u32,
    memory: &ProcMemoryReader,
    profile: &ClientProfile,
) {
    let mapped = address_in_maps(pid, profile.hp_base);
    emit_tool_log_opt(
        Some(app),
        format!(
            "[AutoPot] Memoria: PID={pid} HP addr {:#x} mapped={mapped}",
            profile.hp_base
        ),
    );

    match memory.probe_stats(profile.hp_base) {
        Ok((cur, max, cur_sp, max_sp)) => {
            emit_tool_log_opt(
                Some(app),
                format!("[AutoPot] Probe OK: HP={cur}/{max} SP={cur_sp}/{max_sp}"),
            );
        }
        Err(e) => {
            emit_tool_log_opt(
                Some(app),
                format!(
                    "[AutoPot] Probe falló: {e} (ptrace_scope=1 requiere launcher→wine padre/hijo)"
                ),
            );
        }
    }

    match memory.read_string(profile.name_address, 40) {
        Ok(name) if !name.is_empty() => {
            emit_tool_log_opt(Some(app), format!("[AutoPot] Personaje: '{name}'"));
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
