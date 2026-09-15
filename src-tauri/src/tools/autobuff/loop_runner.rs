use crate::models::autobuff::AutobuffStatusEvent;
use crate::models::memory::{MemoryAccess, ProfileMemory};
use crate::tools::input::emit_status_if_changed;
use crate::tools::memory_sessions::{
    memory_start_error, MemorySessionRegistry, SharedMemorySession, MEMORY_SESSION_MISSING,
};
use crate::utils::{emit_tool_log_opt, EVENT_AUTOBUFF_STATUS};
use ro_tools_core::{AutobuffConfig, AutobuffEngine, ClientProfile};
use ro_tools_linux::ProcessIdentity;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::time::{interval, MissedTickBehavior};

pub(super) struct RunContext {
    pub(super) app: AppHandle,
    pub(super) memory: SharedMemorySession,
    pub(super) writer: crate::tools::input::GatewayWriter,
    pub(super) config: AutobuffConfig,
    pub(super) profile: ClientProfile,
    pub(super) stop_rx: watch::Receiver<bool>,
    pub(super) config_rx: watch::Receiver<AutobuffConfig>,
    pub(super) identity_rx: watch::Receiver<ProcessIdentity>,
    pub(super) status_arc: Arc<Mutex<AutobuffStatusEvent>>,
    pub(super) memory_registry: MemorySessionRegistry,
    pub(super) hp_base: u32,
    pub(super) memory_access: MemoryAccess,
    pub(super) profile_memory: ProfileMemory,
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
        mut identity_rx,
        status_arc,
        memory_registry,
        hp_base,
        mut memory_access,
        mut profile_memory,
    } = context;
    let mut engine = AutobuffEngine::new(memory, writer, config.clone(), profile);
    let mut current = config;
    let mut ticks = ticker(current.delay_ms);
    loop {
        tokio::select! {
            _ = ticks.tick() => {
                if !memory_access.usable() || profile_memory != ProfileMemory::Valid {
                    let error = if !memory_access.usable() {
                        Some(memory_start_error(memory_access))
                    } else {
                        Some("La dirección de memoria del perfil no es válida".into())
                    };
                    emit_status_if_changed(
                        &app,
                        &status_arc,
                        EVENT_AUTOBUFF_STATUS,
                        AutobuffStatusEvent {
                            active: true,
                            active_statuses: 0,
                            last_applied_rule: None,
                            delay_ms: current.delay_ms,
                            error,
                            memory_access: Some(memory_access),
                            profile_memory: Some(profile_memory),
                        },
                    );
                    continue;
                }
                match engine.tick() {
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
                                memory_access: Some(memory_access),
                                profile_memory: Some(profile_memory),
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
                                memory_access: Some(memory_access),
                                profile_memory: Some(profile_memory),
                            },
                        );
                    }
                }
            },
            changed = config_rx.changed() => {
                if changed.is_ok() { current = config_rx.borrow().clone(); engine.update_config(current.clone()); ticks = ticker(current.delay_ms); }
            }
            changed = identity_rx.changed() => {
                if changed.is_ok() {
                    let identity = *identity_rx.borrow();
                    if let Some(session) = memory_registry.get(&identity) {
                        memory_access = session.access();
                        profile_memory = session.profile_memory(Some(hp_base));
                        if memory_access.usable() {
                            engine.replace_memory(SharedMemorySession(session));
                        }
                    } else {
                        memory_access = MemoryAccess::ProcessExitedOrReused;
                        profile_memory = ProfileMemory::InvalidRead;
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            EVENT_AUTOBUFF_STATUS,
                            AutobuffStatusEvent {
                                active: true,
                                delay_ms: current.delay_ms,
                                error: Some(MEMORY_SESSION_MISSING.to_string()),
                                memory_access: Some(memory_access),
                                profile_memory: Some(profile_memory),
                                ..AutobuffStatusEvent::default()
                            },
                        );
                    }
                }
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
            memory_access: Some(memory_access),
            profile_memory: Some(profile_memory),
            ..AutobuffStatusEvent::default()
        },
    );
}
