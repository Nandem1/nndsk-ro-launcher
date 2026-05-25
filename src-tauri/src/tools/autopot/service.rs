use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ro_tools_core::{AutopotConfig, ClientProfile, MemoryReader, ToolsError};
use ro_tools_core::ports::InputWriter;
use ro_tools_linux::{address_in_maps, LazyYdotoolInput, ProcMemoryReader};
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;
use tokio::time::{interval, sleep, Interval, MissedTickBehavior};

use crate::models::autopot::AutopotStatusEvent;
use crate::tools::input::{ensure_ydotoold, YdotoolDaemon};
use crate::utils::{emit_tool_log_opt, EVENT_AUTOPOT_STATUS};

pub struct AutopotHandle {
    stop_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    config_tx: Arc<Mutex<Option<watch::Sender<AutopotConfig>>>>,
    status: Arc<Mutex<AutopotStatusEvent>>,
}

impl Clone for AutopotHandle {
    fn clone(&self) -> Self {
        Self {
            stop_tx: Arc::clone(&self.stop_tx),
            config_tx: Arc::clone(&self.config_tx),
            status: Arc::clone(&self.status),
        }
    }
}

impl AutopotHandle {
    pub fn new() -> Self {
        Self {
            stop_tx: Arc::new(Mutex::new(None)),
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
                .send(config.clamped())
                .map_err(|_| "AutoPot no está activo".to_string()),
            None => Err("AutoPot no está activo".to_string()),
        }
    }

    pub async fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().unwrap().take() {
            let _ = tx.send(true);
        }
        *self.config_tx.lock().unwrap() = None;
        let mut status = self.status.lock().unwrap();
        status.active = false;
    }

    pub async fn start(
        &self,
        app: AppHandle,
        pid: u32,
        config: AutopotConfig,
        profile: ClientProfile,
        ydotoold: Arc<YdotoolDaemon>,
    ) -> Result<(), String> {
        self.stop().await;

        let memory = ProcMemoryReader::open(pid)
            .map_err(|e| format!("No se pudo abrir memoria PID {pid}: {e}"))?;

        log_startup_probe(&app, pid, &memory, &profile);

        let input = Arc::new(LazyYdotoolInput::new());
        let config = config.clamped();

        let (stop_tx, stop_rx) = watch::channel(false);
        let (config_tx, config_rx) = watch::channel(config.clone());
        *self.stop_tx.lock().unwrap() = Some(stop_tx);
        *self.config_tx.lock().unwrap() = Some(config_tx);

        let status_arc = Arc::clone(&self.status);

        emit_tool_log_opt(Some(&app), "[AutoPot] Loop iniciado (ydotool lazy)");

        tokio::spawn(async move {
            super::loop_runner::run(
                app,
                memory,
                input,
                config,
                profile,
                stop_rx,
                config_rx,
                status_arc,
                ydotoold,
            )
            .await;
        });

        sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}

fn log_startup_probe(app: &AppHandle, pid: u32, memory: &ProcMemoryReader, profile: &ClientProfile) {
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
                format!("[AutoPot] Probe falló: {e} (ptrace_scope=1 requiere launcher→wine padre/hijo)"),
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

pub(crate) struct SharedYdotoolInput(pub(crate) Arc<LazyYdotoolInput>);

impl InputWriter for SharedYdotoolInput {
    fn press_key(&self, key: &str) -> Result<(), ToolsError> {
        self.0.press_key(key)
    }
}

pub(crate) fn new_ticker(delay_ms: u64) -> Interval {
    let mut ticker = interval(Duration::from_millis(delay_ms.max(50)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}

pub(crate) fn emit_status_if_changed(
    app: &AppHandle,
    status_arc: &Arc<Mutex<AutopotStatusEvent>>,
    event: AutopotStatusEvent,
) {
    let mut prev = status_arc.lock().unwrap();
    if *prev != event {
        *prev = event.clone();
        drop(prev);
        let _ = app.emit(EVENT_AUTOPOT_STATUS, event);
    }
}

pub(crate) async fn recover_ydotool_on_error(
    app: &AppHandle,
    input: &LazyYdotoolInput,
    ydotoold: &YdotoolDaemon,
    last_recovery: &mut Instant,
    err_msg: &str,
) -> bool {
    if !err_msg.contains("ydotool") || last_recovery.elapsed() < Duration::from_secs(5) {
        return false;
    }

    *last_recovery = Instant::now();
    if ensure_ydotoold(Some(app), ydotoold).await.is_ok() {
        input.reset();
        emit_tool_log_opt(Some(app), "[Input] ydotoold recuperado");
        return true;
    }
    false
}
