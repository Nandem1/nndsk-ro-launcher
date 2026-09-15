use std::sync::{Arc, Mutex};
use std::time::Instant;

use ro_tools_core::{AutopotConfig, AutopotEngine};
use ro_tools_linux::ProcessIdentity;
use tauri::AppHandle;
use tokio::sync::watch;

use crate::models::autopot::AutopotStatusEvent;
use crate::models::memory::{MemoryAccess, ProfileMemory};
use crate::tools::input::{emit_status_if_changed, InputGateway, InputSource};
use crate::tools::memory_sessions::{
    memory_start_error, MemorySessionRegistry, SharedMemorySession, MEMORY_SESSION_MISSING,
};
use crate::utils::EVENT_AUTOPOT_STATUS;

use super::service::new_ticker;

pub struct RunContext {
    pub app: AppHandle,
    pub memory: SharedMemorySession,
    pub writer: crate::tools::input::GatewayWriter,
    pub config: AutopotConfig,
    pub profile: ro_tools_core::ClientProfile,
    pub stop_rx: watch::Receiver<bool>,
    pub config_rx: watch::Receiver<AutopotConfig>,
    pub identity_rx: watch::Receiver<ProcessIdentity>,
    pub status_arc: Arc<Mutex<AutopotStatusEvent>>,
    pub gateway: InputGateway,
    pub memory_registry: MemorySessionRegistry,
    pub hp_base: u32,
    pub memory_access: MemoryAccess,
    pub profile_memory: ProfileMemory,
}

pub async fn run(context: RunContext) {
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
        gateway,
        memory_registry,
        hp_base,
        mut memory_access,
        mut profile_memory,
    } = context;
    let mut engine = AutopotEngine::new(memory, writer, config.clone(), profile);
    let mut current_config = config;
    let mut ticker = new_ticker(current_config.delay_ms);
    let mut metrics_ticker = tokio::time::interval(std::time::Duration::from_secs(10));
    metrics_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    metrics_ticker.tick().await;
    let mut scan_periods_us = Vec::new();
    let mut scan_durations_us = Vec::new();
    let mut last_scan: Option<Instant> = None;
    let mut tick_count: u64 = 0;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if !memory_access.usable() || profile_memory != ProfileMemory::Valid {
                    let prev = status_arc.lock().unwrap().clone();
                    let error = if !memory_access.usable() {
                        Some(memory_start_error(memory_access))
                    } else {
                        Some("La dirección de memoria del perfil no es válida".into())
                    };
                    emit_status_if_changed(
                        &app,
                        &status_arc,
                        EVENT_AUTOPOT_STATUS,
                        AutopotStatusEvent {
                            active: true,
                            effective_delay_ms: current_config.delay_ms,
                            cur_hp: prev.cur_hp,
                            max_hp: prev.max_hp,
                            cur_sp: prev.cur_sp,
                            max_sp: prev.max_sp,
                            character_name: prev.character_name,
                            hp_percent: current_config.hp_percent,
                            sp_percent: current_config.sp_percent,
                            error,
                            memory_access: Some(memory_access),
                            profile_memory: Some(profile_memory),
                        },
                    );
                    continue;
                }

                tick_count += 1;
                let scan_started = Instant::now();
                if let Some(previous) = last_scan.replace(scan_started) {
                    scan_periods_us.push(scan_started.duration_since(previous).as_micros() as u64);
                }
                let tick_result = engine.tick().map_err(|error| error.to_string());
                scan_durations_us.push(scan_started.elapsed().as_micros() as u64);

                match tick_result {
                    Ok(tick) => {
                        if tick.potted_hp || tick.potted_sp {
                            crate::utils::emit_tool_log_opt(
                                Some(&app),
                                format!(
                                    "[AutoPot] tick#{tick_count} HP={} SP={} proactivo={}",
                                    if tick.potted_hp { "sí" } else { "—" },
                                    if tick.potted_sp { "sí" } else { "—" },
                                    if tick.proactive_hp_pulse { "sí" } else { "—" },
                                ),
                            );
                        }
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            EVENT_AUTOPOT_STATUS,
                            AutopotStatusEvent {
                                active: true,
                                effective_delay_ms: current_config.delay_ms,
                                cur_hp: tick.cur_hp,
                                max_hp: tick.max_hp,
                                cur_sp: tick.cur_sp,
                                max_sp: tick.max_sp,
                                hp_percent: current_config.hp_percent,
                                sp_percent: current_config.sp_percent,
                                character_name: tick.character_name,
                                error: None,
                                memory_access: Some(memory_access),
                                profile_memory: Some(profile_memory),
                            },
                        );
                    }
                    Err(err_msg) => {
                        let prev = status_arc.lock().unwrap().clone();
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            EVENT_AUTOPOT_STATUS,
                            AutopotStatusEvent {
                                active: true,
                                effective_delay_ms: current_config.delay_ms,
                                cur_hp: prev.cur_hp,
                                max_hp: prev.max_hp,
                                cur_sp: prev.cur_sp,
                                max_sp: prev.max_sp,
                                character_name: prev.character_name,
                                error: Some(err_msg.clone()),
                                hp_percent: current_config.hp_percent,
                                sp_percent: current_config.sp_percent,
                                memory_access: Some(memory_access),
                                profile_memory: Some(profile_memory),
                            },
                        );
                        crate::utils::emit_tool_log_opt(
                            Some(&app),
                            format!("[AutoPot] ERROR tick: {err_msg}"),
                        );
                    }
                }
            }
            changed = config_rx.changed() => {
                if changed.is_ok() {
                    current_config = config_rx.borrow().clone().clamped();
                    engine.update_config(current_config.clone());
                    ticker = new_ticker(current_config.delay_ms);
                }
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
                            EVENT_AUTOPOT_STATUS,
                            AutopotStatusEvent {
                                active: true,
                                effective_delay_ms: current_config.delay_ms,
                                hp_percent: current_config.hp_percent,
                                sp_percent: current_config.sp_percent,
                                error: Some(MEMORY_SESSION_MISSING.to_string()),
                                memory_access: Some(memory_access),
                                profile_memory: Some(profile_memory),
                                ..status_arc.lock().unwrap().clone()
                            },
                        );
                    }
                }
            }
            _ = metrics_ticker.tick() => {
                log_metrics(
                    &app,
                    &gateway,
                    current_config.delay_ms,
                    &mut scan_periods_us,
                    &mut scan_durations_us,
                    false,
                );
            }
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() {
                    break;
                }
            }
        }
    }

    log_metrics(
        &app,
        &gateway,
        current_config.delay_ms,
        &mut scan_periods_us,
        &mut scan_durations_us,
        true,
    );
    crate::utils::emit_tool_log_opt(Some(&app), "[AutoPot] Loop detenido");
    let idle = AutopotStatusEvent {
        active: false,
        effective_delay_ms: current_config.delay_ms,
        hp_percent: current_config.hp_percent,
        sp_percent: current_config.sp_percent,
        error: None,
        memory_access: Some(memory_access),
        profile_memory: Some(profile_memory),
        ..AutopotStatusEvent::default()
    };
    emit_status_if_changed(&app, &status_arc, EVENT_AUTOPOT_STATUS, idle);
}

fn log_metrics(
    app: &AppHandle,
    gateway: &InputGateway,
    effective_delay_ms: u64,
    scan_periods_us: &mut Vec<u64>,
    scan_durations_us: &mut Vec<u64>,
    final_window: bool,
) {
    let input = gateway.metrics(InputSource::Autopot);
    let line = format!(
        "{} effective_delay_ms={} autopot_read_period_us[p50/p95/p99]={}/{}/{} scan_duration_us[p50/p95/p99]={}/{}/{}",
        input.log_line(InputSource::Autopot, final_window),
        effective_delay_ms,
        percentile(scan_periods_us, 50),
        percentile(scan_periods_us, 95),
        percentile(scan_periods_us, 99),
        percentile(scan_durations_us, 50),
        percentile(scan_durations_us, 95),
        percentile(scan_durations_us, 99),
    );
    crate::utils::emit_tool_log_opt(Some(app), line);
    scan_periods_us.clear();
    scan_durations_us.clear();
}

fn percentile(values: &[u64], pct: u8) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * (pct as f64 / 100.0)).round() as usize;
    sorted[index]
}
