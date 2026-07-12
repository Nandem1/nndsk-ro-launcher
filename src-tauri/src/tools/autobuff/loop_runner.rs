use crate::models::autobuff::AutobuffStatusEvent;
use crate::tools::input::{
    emit_status_if_changed, recover_ydotool_on_error, InputGateway, YdotoolDaemon,
};
use crate::utils::{emit_tool_log_opt, EVENT_AUTOBUFF_STATUS};
use ro_tools_core::{AutobuffConfig, AutobuffEngine, ClientProfile};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::time::{interval, MissedTickBehavior};

pub(super) struct RunContext {
    pub(super) app: AppHandle,
    pub(super) memory: ro_tools_linux::ProcMemoryReader,
    pub(super) writer: crate::tools::input::GatewayWriter,
    pub(super) config: AutobuffConfig,
    pub(super) profile: ClientProfile,
    pub(super) stop_rx: watch::Receiver<bool>,
    pub(super) config_rx: watch::Receiver<AutobuffConfig>,
    pub(super) status_arc: Arc<Mutex<AutobuffStatusEvent>>,
    pub(super) gateway: InputGateway,
    pub(super) ydotoold: Arc<YdotoolDaemon>,
}

fn ticker(delay_ms: u64) -> tokio::time::Interval {
    let mut value = interval(Duration::from_millis(delay_ms.max(100)));
    value.set_missed_tick_behavior(MissedTickBehavior::Skip);
    value
}

pub(super) async fn run(context: RunContext) {
    let RunContext {
        app,
        memory,
        writer,
        config,
        profile,
        mut stop_rx,
        mut config_rx,
        status_arc,
        gateway,
        ydotoold,
    } = context;
    let engine = Arc::new(Mutex::new(AutobuffEngine::new(
        memory,
        writer,
        config.clone(),
        profile,
    )));
    let mut current = config;
    let mut ticks = ticker(current.delay_ms);
    let mut last_recovery = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);
    loop {
        tokio::select! {
            _ = ticks.tick() => { let engine = Arc::clone(&engine); match tokio::task::spawn_blocking(move || engine.lock().unwrap().tick()).await {
                Ok(Ok(tick)) => { if let Some(rule) = &tick.applied_rule { emit_tool_log_opt(Some(&app), format!("[AutoBuff] Aplicado: {rule}")); } emit_status_if_changed(&app, &status_arc, EVENT_AUTOBUFF_STATUS, AutobuffStatusEvent { active: true, active_statuses: tick.active_statuses, last_applied_rule: tick.applied_rule, delay_ms: current.delay_ms, error: None }); }
                Ok(Err(error)) => { let message = error.to_string(); recover_ydotool_on_error(&app, &gateway, ydotoold.as_ref(), &mut last_recovery, &message, "[Input] ydotoold recuperado").await; emit_status_if_changed(&app, &status_arc, EVENT_AUTOBUFF_STATUS, AutobuffStatusEvent { active: true, active_statuses: 0, last_applied_rule: None, delay_ms: current.delay_ms, error: Some(message) }); }
                Err(error) => emit_tool_log_opt(Some(&app), format!("[AutoBuff] ERROR tick: {error}")),
            } }
            changed = config_rx.changed() => {
                if changed.is_ok() { current = config_rx.borrow().clone(); engine.lock().unwrap().update_config(current.clone()); ticks = ticker(current.delay_ms); }
            }
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() { break; }
            }
        }
    }
    emit_tool_log_opt(Some(&app), "[AutoBuff] Loop detenido");
    emit_status_if_changed(
        &app,
        &status_arc,
        EVENT_AUTOBUFF_STATUS,
        AutobuffStatusEvent {
            active: false,
            delay_ms: current.delay_ms,
            ..AutobuffStatusEvent::default()
        },
    );
}
