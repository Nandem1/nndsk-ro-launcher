use crate::models::autobuff::AutobuffStatusEvent;
use crate::tools::input::emit_status_if_changed;
use crate::utils::{emit_tool_log_opt, EVENT_AUTOBUFF_STATUS};
use ro_tools_core::{AutobuffConfig, AutobuffEngine, ClientProfile};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    } = context;
    let mut engine = AutobuffEngine::new(memory, writer, config.clone(), profile);
    let mut current = config;
    let mut ticks = ticker(current.delay_ms);
    loop {
        tokio::select! {
            _ = ticks.tick() => match engine.tick() {
                Ok(tick) => {
                    if let Some(rule) = &tick.applied_rule {
                        emit_tool_log_opt(Some(&app), format!("[AutoBuff] Aplicado: {rule}"));
                    }
                    emit_status_if_changed(
                        &app,
                        &status_arc,
                        EVENT_AUTOBUFF_STATUS,
                        AutobuffStatusEvent {
                            active: true,
                            active_statuses: tick.active_statuses,
                            last_applied_rule: tick.applied_rule,
                            delay_ms: current.delay_ms,
                            error: None,
                        },
                    );
                }
                Err(error) => {
                    emit_status_if_changed(
                        &app,
                        &status_arc,
                        EVENT_AUTOBUFF_STATUS,
                        AutobuffStatusEvent {
                            active: true,
                            active_statuses: 0,
                            last_applied_rule: None,
                            delay_ms: current.delay_ms,
                            error: Some(error.to_string()),
                        },
                    );
                }
            },
            changed = config_rx.changed() => {
                if changed.is_ok() { current = config_rx.borrow().clone(); engine.update_config(current.clone()); ticks = ticker(current.delay_ms); }
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
