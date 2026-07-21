use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ro_tools_core::{SpammerConfig, SpammerTick};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::{interval, sleep, Instant as TokioInstant, MissedTickBehavior};

use super::gear::{self, GearMode};
use super::timing::SpammerTimingPlan;
use crate::models::spammer::SpammerStatusEvent;
use crate::tools::input::{emit_status_if_changed, GatewayWriter, InputGateway, InputSource};
use crate::utils::{emit_tool_log_opt, EVENT_SPAMMER_STATUS};

const INPUTD_READY_TIMEOUT_SECS: u64 = 5;
const STALL_LOG_COOLDOWN: Duration = Duration::from_secs(2);
const INFLIGHT_WARN_AFTER: Duration = Duration::from_millis(250);
const INFLIGHT_WARN_REPEAT: Duration = Duration::from_secs(1);
const TIMER_STALL_US: u64 = 25_000;
const SPAWN_STALL_US: u64 = 10_000;
const TASK_STALL_US: u64 = 60_000;
const JOIN_STALL_US: u64 = 25_000;
const INFLIGHT_WAITING_FOR_SPAWN: u8 = 0;
const INFLIGHT_RUNNING_TASK: u8 = 1;
const INFLIGHT_FINISHED_TASK: u8 = 2;

struct CycleTaskResult {
    attempt: u64,
    log_key: String,
    tick_result: Result<SpammerTick, String>,
    timer_late_us: u64,
    spawn_wait_us: u64,
    task_us: u64,
    finished_at: Instant,
}

struct InflightCycle {
    attempt: u64,
    key: String,
    dispatched_at: Instant,
    stage: Arc<AtomicU8>,
}

impl InflightCycle {
    fn stage_label(&self) -> &'static str {
        match self.stage.load(Ordering::Acquire) {
            INFLIGHT_WAITING_FOR_SPAWN => "spawn",
            INFLIGHT_RUNNING_TASK => "task_or_input",
            INFLIGHT_FINISHED_TASK => "join",
            _ => "unknown",
        }
    }
}

struct MetricsLogContext {
    timing: SpammerTimingPlan,
    effective_delay_ms: u64,
    elapsed: Duration,
    cycle_count: u64,
    cycle_attempt: u64,
    active: bool,
}

#[derive(Default)]
struct LoopTimingWindow {
    timer_late_us: Vec<u64>,
    spawn_wait_us: Vec<u64>,
    task_us: Vec<u64>,
    join_wait_us: Vec<u64>,
    completed: u64,
    skipped_while_active: u64,
    suppressed_stall_logs: u64,
}

impl LoopTimingWindow {
    fn record(&mut self, result: &CycleTaskResult, join_wait_us: u64, same_key_active: bool) {
        self.timer_late_us.push(result.timer_late_us);
        self.spawn_wait_us.push(result.spawn_wait_us);
        self.task_us.push(result.task_us);
        self.join_wait_us.push(join_wait_us);
        if result.tick_result.as_ref().is_ok_and(|tick| tick.cycled) {
            self.completed += 1;
        } else if same_key_active && result.tick_result.is_ok() {
            self.skipped_while_active += 1;
        }
    }

    fn log_fields_and_reset(&mut self) -> String {
        let timer = timing_stats(&self.timer_late_us);
        let spawn = timing_stats(&self.spawn_wait_us);
        let task = timing_stats(&self.task_us);
        let join = timing_stats(&self.join_wait_us);
        let line = format!(
            "loop_samples={} timer_late_us[p50/p95/p99/max]={}/{}/{}/{} spawn_wait_us[p50/p95/p99/max]={}/{}/{}/{} task_us[p50/p95/p99/max]={}/{}/{}/{} join_wait_us[p50/p95/p99/max]={}/{}/{}/{} completed={} skipped_while_active={} suppressed_stall_logs={}",
            self.task_us.len(),
            timer.0,
            timer.1,
            timer.2,
            timer.3,
            spawn.0,
            spawn.1,
            spawn.2,
            spawn.3,
            task.0,
            task.1,
            task.2,
            task.3,
            join.0,
            join.1,
            join.2,
            join.3,
            self.completed,
            self.skipped_while_active,
            self.suppressed_stall_logs,
        );
        self.timer_late_us.clear();
        self.spawn_wait_us.clear();
        self.task_us.clear();
        self.join_wait_us.clear();
        self.completed = 0;
        self.skipped_while_active = 0;
        self.suppressed_stall_logs = 0;
        line
    }
}

struct SpamReleaseGuard {
    writer: GatewayWriter,
}

impl Drop for SpamReleaseGuard {
    fn drop(&mut self) {
        let _ = self.writer.release_spam();
    }
}

#[derive(Debug)]
enum InputdMsg {
    Ready,
    TriggerHeld { key: String, held: bool },
    Fatal(String),
}

fn parse_line(line: &str) -> Option<InputdMsg> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    match v.get("type")?.as_str()? {
        "ready" => Some(InputdMsg::Ready),
        "trigger" => {
            let key = v.get("key")?.as_str()?.to_string();
            Some(InputdMsg::TriggerHeld {
                key,
                held: v.get("held")?.as_bool()?,
            })
        }
        "fatal" => Some(InputdMsg::Fatal(
            v.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("fatal")
                .to_string(),
        )),
        _ => None,
    }
}

fn find_ro_inputd() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let plain = dir.join("ro-inputd");
            if plain.exists() {
                return plain;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name == "ro-inputd" || name.starts_with("ro-inputd-") {
                        return entry.path();
                    }
                }
            }
        }
    }
    std::path::PathBuf::from("ro-inputd")
}

fn build_status(
    config: &SpammerConfig,
    active_key: &str,
    cycle_count: u64,
    error: Option<String>,
    lifecycle: (bool, bool),
    gear_mode: Option<&str>,
    timing: SpammerTimingPlan,
) -> SpammerStatusEvent {
    let (active, armed) = lifecycle;
    SpammerStatusEvent {
        active,
        effective_delay_ms: timing.post_delay_ms,
        armed,
        spamming: !active_key.is_empty(),
        key: active_key.to_string(),
        delay_ms: config.delay_ms,
        cycle_count,
        error,
        gear_mode: gear_mode.map(str::to_string),
    }
}

pub async fn run(
    app: AppHandle,
    writer: crate::tools::input::GatewayWriter,
    config: SpammerConfig,
    timing: SpammerTimingPlan,
    mut stop_rx: watch::Receiver<bool>,
    status_arc: Arc<Mutex<SpammerStatusEvent>>,
    gateway: InputGateway,
) {
    let config = config.clamped();
    let effective_delay_ms = timing.post_delay_ms;
    let triggers_arg = config.keys.join(",");

    let inputd_path = find_ro_inputd();

    let mut child = match tokio::process::Command::new(&inputd_path)
        .arg("--triggers")
        .arg(&triggers_arg)
        .arg("--json")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("[Spammer] ro-inputd no encontrado ({inputd_path:?}): {e}");
            emit_tool_log_opt(Some(&app), &msg);
            emit_status_if_changed(
                &app,
                &status_arc,
                EVENT_SPAMMER_STATUS,
                build_status(&config, "", 0, Some(msg), (false, false), None, timing),
            );
            return;
        }
    };

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut lines = BufReader::new(child.stdout.take().expect("stdout piped")).lines();

    emit_tool_log_opt(
        Some(&app),
        format!("[Spammer] Esperando ro-inputd (triggers: {triggers_arg})..."),
    );

    let cleanup_writer = writer.clone();
    let _release_guard = SpamReleaseGuard {
        writer: cleanup_writer.clone(),
    };
    let cycle_writer = writer;
    let mut cycle_tasks = JoinSet::new();
    let cycle_delay = Duration::from_millis(effective_delay_ms);
    let cycle_sleep = sleep(Duration::ZERO);
    tokio::pin!(cycle_sleep);
    let mut cycle_armed = false;
    let mut next_cycle_due: Option<TokioInstant> = None;
    let inflight_watchdog = sleep(Duration::ZERO);
    tokio::pin!(inflight_watchdog);
    let mut inflight_watchdog_armed = false;
    let mut inflight_cycle: Option<InflightCycle> = None;
    let mut metrics_ticker = interval(Duration::from_secs(10));
    metrics_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    metrics_ticker.tick().await;
    let mut last_log_cycle: u64 = 0;
    let mut cycle_count: u64 = 0;
    let mut cycle_attempt: u64 = 0;
    let mut loop_timing = LoopTimingWindow::default();
    let mut active_key = String::new();
    let mut held_keys: Vec<String> = Vec::new();
    let mut gear_mode: Option<&'static str> = None;
    let mut ready_received = false;
    let mut terminal_error: Option<String> = None;
    let session_started = Instant::now();
    let mut last_stall_log = Instant::now()
        .checked_sub(STALL_LOG_COOLDOWN)
        .unwrap_or_else(Instant::now);
    let mut last_status_emit = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);

    let ready_timeout = tokio::time::sleep(Duration::from_secs(INPUTD_READY_TIMEOUT_SECS));
    tokio::pin!(ready_timeout);

    'main: loop {
        tokio::select! {
            // A physical release or stop must win over a click whose timer is also ready.
            biased;
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    break 'main;
                }
            }
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => match parse_line(&line) {
                        Some(InputdMsg::Ready) => {
                            ready_received = true;
                            emit_tool_log_opt(Some(&app), "[Spammer] ro-inputd listo — grab activo");
                            if let Err(error) = cleanup_writer.release_spam() {
                                let msg = format!("no se pudo limpiar la tecla del spammer: {error}");
                                emit_tool_log_opt(Some(&app), format!("[Spammer] ERROR input: {msg}"));
                                terminal_error = Some(msg);
                                break 'main;
                            }
                            emit_status_if_changed(
                                &app,
                                &status_arc,
                                EVENT_SPAMMER_STATUS,
                                build_status(
                                    &config,
                                    "",
                                    0,
                                    None,
                                    (true, true),
                                    gear_mode,
                                    timing,
                                ),
                            );
                        }
                        Some(InputdMsg::TriggerHeld { key, held }) if ready_received => {
                            let previous_active_key = active_key.clone();
                            let was_held = held_keys.iter().any(|k| k == &key);

                            if held {
                                if !was_held {
                                    held_keys.push(key.clone());
                                }
                                active_key = key.clone();
                                if previous_active_key.is_empty() {
                                    cycle_count = 0;
                                    last_log_cycle = 0;
                                    emit_tool_log_opt(
                                        Some(&app),
                                        format!("[Spammer] Spam activo ({key})"),
                                    );
                                    if cycle_tasks.is_empty() {
                                        let due = TokioInstant::now();
                                        cycle_sleep.as_mut().reset(due);
                                        next_cycle_due = Some(due);
                                        cycle_armed = true;
                                    } else {
                                        // The cancelled prior attempt must be observed before a
                                        // new key can dispatch, preserving one in-flight cycle.
                                        next_cycle_due = None;
                                        cycle_armed = false;
                                    }
                                }
                            } else {
                                held_keys.retain(|k| k != &key);
                                if active_key == key {
                                    if let Some(next) = held_keys.last() {
                                        active_key = next.clone();
                                        cycle_count = 0;
                                        last_log_cycle = 0;
                                    } else {
                                        active_key.clear();
                                    }
                                }
                            }

                            if active_key != previous_active_key && !previous_active_key.is_empty() {
                                if let Err(error) = cleanup_writer.release_spam() {
                                    let msg = format!("no se pudo soltar la tecla del spammer: {error}");
                                    emit_tool_log_opt(Some(&app), format!("[Spammer] ERROR input: {msg}"));
                                    terminal_error = Some(msg);
                                    break 'main;
                                }
                            }
                            if active_key.is_empty() {
                                cycle_armed = false;
                                next_cycle_due = None;
                            }

                            // Log only after the latency-sensitive cancellation/release path.
                            emit_tool_log_opt(
                                Some(&app),
                                format!(
                                    "[Spammer][diag] trigger elapsed_ms={} key={} held={} was_held={} active_before={} active_after={} session_attempts={} hold_cycles={}",
                                    session_started.elapsed().as_millis(),
                                    key,
                                    held,
                                    was_held,
                                    if previous_active_key.is_empty() {
                                        "none"
                                    } else {
                                        previous_active_key.as_str()
                                    },
                                    if active_key.is_empty() {
                                        "none"
                                    } else {
                                        active_key.as_str()
                                    },
                                    cycle_attempt,
                                    cycle_count,
                                ),
                            );

                            if active_key != previous_active_key {
                                emit_status_if_changed(
                                    &app,
                                    &status_arc,
                                    EVENT_SPAMMER_STATUS,
                                    build_status(
                                        &config,
                                        &active_key,
                                        cycle_count,
                                        None,
                                        (true, true),
                                        gear_mode,
                                        timing,
                                    ),
                                );
                            }

                            // Gear switch: cada regla es edge-triggered por su propia tecla.
                            // Press fresco → equipa ATK; release de una tecla presionada → DEF.
                            let fresh_press = held && !was_held;
                            let fresh_release = !held && was_held;
                            if config.gear_switch.enabled && (fresh_press || fresh_release) {
                                if let Some(rule) = config.gear_switch.rule_for(&key) {
                                    // Gear may block for multiple key presses. Do not leave the
                                    // synthetic skill held throughout that operation.
                                    if let Err(error) = cleanup_writer.release_spam() {
                                        let msg = format!("no se pudo pausar el spammer para gear: {error}");
                                        emit_tool_log_opt(Some(&app), format!("[Spammer] ERROR input: {msg}"));
                                        terminal_error = Some(msg);
                                        break 'main;
                                    }
                                    // Keep the existing deadline: Gear must not create an early
                                    // click; this select branch cannot poll it during the await.

                                    let (keys, mode, label) = if fresh_press {
                                        (rule.atk_keys.clone(), GearMode::Atk, "ATK")
                                    } else {
                                        (rule.def_keys.clone(), GearMode::Def, "DEF")
                                    };
                                    if !keys.is_empty() {
                                        let switch_delay = config.gear_switch.switch_delay_ms;
                                        let writer = match gateway.writer(InputSource::Gear, switch_delay) {
                                            Ok(writer) => writer,
                                            Err(error) => {
                                                emit_tool_log_opt(
                                                    Some(&app),
                                                    format!("[Spammer] ERROR gear input: {error}"),
                                                );
                                                continue 'main;
                                            }
                                        };
                                        let keys_log = keys.join("+");
                                        let equip_result = tokio::task::spawn_blocking(
                                            move || gear::equip(&writer, &keys, switch_delay),
                                        )
                                        .await;
                                        match equip_result {
                                            Ok(Ok(())) => {
                                                gear_mode = Some(mode.as_str());
                                                emit_tool_log_opt(
                                                    Some(&app),
                                                    format!(
                                                        "[Spammer] Gear {label} {key}→{keys_log}"
                                                    ),
                                                );
                                            }
                                            Ok(Err(error)) => emit_tool_log_opt(
                                                Some(&app),
                                                format!("[Spammer] ERROR gear input: {error}"),
                                            ),
                                            Err(e) => {
                                                emit_tool_log_opt(
                                                    Some(&app),
                                                    format!("[Spammer] ERROR gear (join): {e}"),
                                                );
                                            }
                                        }
                                    } else {
                                        gear_mode = Some(mode.as_str());
                                    }
                                }
                            }

                            if config.gear_switch.enabled && (fresh_press || fresh_release) {
                                emit_status_if_changed(
                                    &app,
                                    &status_arc,
                                    EVENT_SPAMMER_STATUS,
                                    build_status(
                                        &config,
                                        &active_key,
                                        cycle_count,
                                        None,
                                        (true, true),
                                        gear_mode,
                                        timing,
                                    ),
                                );
                            }
                        }
                        Some(InputdMsg::Fatal(msg)) => {
                            emit_tool_log_opt(Some(&app), format!("[Spammer] Fatal ro-inputd: {msg}"));
                            emit_status_if_changed(
                                &app,
                                &status_arc,
                                EVENT_SPAMMER_STATUS,
                                build_status(
                                    &config,
                                    &active_key,
                                    cycle_count,
                                    Some(msg),
                                    (false, false),
                                    None,
                                    timing,
                                ),
                            );
                            break 'main;
                        }
                        _ => {}
                    }
                    _ => {
                        let msg = "[Spammer] ro-inputd terminó inesperadamente".to_string();
                        emit_tool_log_opt(Some(&app), &msg);
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            EVENT_SPAMMER_STATUS,
                            build_status(
                                &config,
                                &active_key,
                                cycle_count,
                                Some(msg),
                                (false, false),
                                None,
                                timing,
                            ),
                        );
                        break 'main;
                    }
                }
            }
            _ = &mut cycle_sleep, if ready_received && cycle_armed => {
                cycle_armed = false;
                let timer_fired_at = TokioInstant::now();
                let scheduled_deadline = next_cycle_due.take().unwrap_or(timer_fired_at);
                let timer_late_us = duration_us(
                    timer_fired_at.saturating_duration_since(scheduled_deadline),
                );
                if !active_key.is_empty() {
                    cycle_attempt += 1;
                    let attempt = cycle_attempt;
                    let tick_key = active_key.clone();
                    let log_key = tick_key.clone();
                    let tick_writer = cycle_writer.clone();
                    let dispatched_at = Instant::now();
                    // Capture cancellation generation and deadline before entering the
                    // blocking pool. A delayed closure must not become a fresh click
                    // after the physical trigger was already released.
                    let ticket = tick_writer.spam_cycle_ticket(Some(
                        dispatched_at + Duration::from_millis(timing.deadline_budget_ms),
                    ));
                    let inflight_stage = Arc::new(AtomicU8::new(INFLIGHT_WAITING_FOR_SPAWN));
                    inflight_cycle = Some(InflightCycle {
                        attempt,
                        key: log_key.clone(),
                        dispatched_at,
                        stage: Arc::clone(&inflight_stage),
                    });
                    inflight_watchdog
                        .as_mut()
                        .reset(TokioInstant::now() + INFLIGHT_WARN_AFTER);
                    inflight_watchdog_armed = true;
                    cycle_tasks.spawn_blocking(move || {
                        let task_started = Instant::now();
                        inflight_stage.store(INFLIGHT_RUNNING_TASK, Ordering::Release);
                        let spawn_wait_us = duration_us(task_started.duration_since(dispatched_at));
                        let tick_result = tick_writer
                            .spam_cycle_with_ticket(&tick_key, ticket)
                            .map(|cycled| SpammerTick { cycled })
                            .map_err(|error| error.to_string());
                        let finished_at = Instant::now();
                        inflight_stage.store(INFLIGHT_FINISHED_TASK, Ordering::Release);
                        CycleTaskResult {
                            attempt,
                            log_key,
                            tick_result,
                            timer_late_us,
                            spawn_wait_us,
                            task_us: duration_us(finished_at.duration_since(task_started)),
                            finished_at,
                        }
                    });
                }
            }
            cycle_result = cycle_tasks.join_next(), if !cycle_tasks.is_empty() => {
                let result = match cycle_result {
                    Some(Ok(result)) => result,
                    Some(Err(error)) => {
                        let err_msg = format!("worker del ciclo terminó con error: {error}");
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            EVENT_SPAMMER_STATUS,
                            build_status(
                                &config,
                                &active_key,
                                cycle_count,
                                Some(err_msg.clone()),
                                (true, true),
                                gear_mode,
                                timing,
                            ),
                        );
                        terminal_error = Some(err_msg);
                        break 'main;
                    }
                    None => continue 'main,
                };
                inflight_watchdog_armed = false;
                inflight_cycle = None;
                let join_wait_us = duration_us(result.finished_at.elapsed());
                let same_key_active = active_key == result.log_key;
                loop_timing.record(&result, join_wait_us, same_key_active);
                if let Some(flags) = stall_flags(&result, join_wait_us, same_key_active) {
                    if last_stall_log.elapsed() >= STALL_LOG_COOLDOWN {
                        last_stall_log = Instant::now();
                        emit_tool_log_opt(
                            Some(&app),
                            format!(
                                "[Spammer][diag] stall elapsed_ms={} attempt={} flags={} timer_late_us={} spawn_wait_us={} task_us={} join_wait_us={} post_delay_ms={} active={} hold_cycles={}",
                                session_started.elapsed().as_millis(),
                                result.attempt,
                                flags,
                                result.timer_late_us,
                                result.spawn_wait_us,
                                result.task_us,
                                join_wait_us,
                                effective_delay_ms,
                                if active_key.is_empty() { "none" } else { active_key.as_str() },
                                cycle_count,
                            ),
                        );
                    } else {
                        loop_timing.suppressed_stall_logs += 1;
                    }
                }
                let CycleTaskResult {
                    log_key,
                    tick_result,
                    ..
                } = result;

                match tick_result {
                    Ok(tick) if tick.cycled => {
                        cycle_count += 1;
                        let should_log = cycle_count == 1
                            || cycle_count.saturating_sub(last_log_cycle) >= 100;
                        if should_log {
                            last_log_cycle = cycle_count;
                            emit_tool_log_opt(
                                Some(&app),
                                format!("[Spammer] cycle #{cycle_count} {log_key} + click"),
                            );
                        }
                    }
                    Err(err_msg) => {
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            EVENT_SPAMMER_STATUS,
                            build_status(
                                &config,
                                &active_key,
                                cycle_count,
                                Some(err_msg.clone()),
                                (true, true),
                                gear_mode,
                                timing,
                            ),
                        );
                        terminal_error = Some(err_msg);
                        break 'main;
                    }
                    _ => {}
                }

                if !active_key.is_empty() {
                    // Start the post-cycle pause after diagnostics/status work so
                    // observability can never shorten the configured delay.
                    let due = next_cycle_deadline(TokioInstant::now(), cycle_delay);
                    cycle_sleep.as_mut().reset(due);
                    next_cycle_due = Some(due);
                    cycle_armed = true;
                }

                if last_status_emit.elapsed() >= Duration::from_millis(250) {
                    last_status_emit = Instant::now();
                    emit_status_if_changed(
                        &app,
                        &status_arc,
                        EVENT_SPAMMER_STATUS,
                        build_status(
                            &config,
                            &active_key,
                            cycle_count,
                            None,
                            (true, true),
                            gear_mode,
                            timing,
                        ),
                    );
                }
            }
            _ = &mut inflight_watchdog, if inflight_watchdog_armed => {
                if let Some(inflight) = inflight_cycle.as_ref() {
                    emit_tool_log_opt(
                        Some(&app),
                        format!(
                            "[Spammer][diag] inflight_stall elapsed_ms={} attempt={} stage={} inflight_ms={} key={} active={} session_attempts={} hold_cycles={}",
                            session_started.elapsed().as_millis(),
                            inflight.attempt,
                            inflight.stage_label(),
                            inflight.dispatched_at.elapsed().as_millis(),
                            inflight.key,
                            if active_key.is_empty() { "none" } else { active_key.as_str() },
                            cycle_attempt,
                            cycle_count,
                        ),
                    );
                    inflight_watchdog
                        .as_mut()
                        .reset(TokioInstant::now() + INFLIGHT_WARN_REPEAT);
                } else {
                    inflight_watchdog_armed = false;
                }
            }
            _ = metrics_ticker.tick() => {
                if !active_key.is_empty() && !cycle_armed && cycle_tasks.is_empty() {
                    emit_tool_log_opt(
                        Some(&app),
                        format!(
                            "[Spammer][diag] invariant=active_without_timer_or_task elapsed_ms={} active={} session_attempts={} hold_cycles={}",
                            session_started.elapsed().as_millis(),
                            active_key,
                            cycle_attempt,
                            cycle_count,
                        ),
                    );
                }
                if cycle_tasks.len() > 1 {
                    emit_tool_log_opt(
                        Some(&app),
                        format!(
                            "[Spammer][diag] invariant=multiple_inflight_tasks elapsed_ms={} inflight={} active={}",
                            session_started.elapsed().as_millis(),
                            cycle_tasks.len(),
                            if active_key.is_empty() { "none" } else { active_key.as_str() },
                        ),
                    );
                }
                log_metrics(
                    &app,
                    &gateway,
                    MetricsLogContext {
                        timing,
                        effective_delay_ms,
                        elapsed: session_started.elapsed(),
                        cycle_count,
                        cycle_attempt,
                        active: !active_key.is_empty(),
                    },
                    &mut loop_timing,
                    false,
                );
            }
            _ = &mut ready_timeout, if !ready_received => {
                let msg = "[Spammer] ro-inputd no respondió (timeout)".to_string();
                emit_tool_log_opt(Some(&app), &msg);
                emit_status_if_changed(
                    &app,
                    &status_arc,
                    EVENT_SPAMMER_STATUS,
                    build_status(
                        &config,
                        &active_key,
                        cycle_count,
                        Some(msg),
                        (false, false),
                        None,
                        timing,
                    ),
                );
                break 'main;
            }
        }
    }

    if let Err(error) = cleanup_writer.release_spam() {
        let msg = format!("no se pudo liberar el spammer al detenerse: {error}");
        emit_tool_log_opt(Some(&app), format!("[Spammer] ERROR input: {msg}"));
        terminal_error.get_or_insert(msg);
    }
    cycle_tasks.abort_all();
    while cycle_tasks.join_next().await.is_some() {}

    let _ = stdin.write_all(b"{\"type\":\"stop\"}\n").await;
    let _ = stdin.flush().await;
    drop(stdin);
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;

    log_metrics(
        &app,
        &gateway,
        MetricsLogContext {
            timing,
            effective_delay_ms,
            elapsed: session_started.elapsed(),
            cycle_count,
            cycle_attempt,
            active: false,
        },
        &mut loop_timing,
        true,
    );
    emit_tool_log_opt(Some(&app), "[Spammer] Loop detenido");
    emit_status_if_changed(
        &app,
        &status_arc,
        EVENT_SPAMMER_STATUS,
        build_status(
            &config,
            "",
            cycle_count,
            terminal_error,
            (false, false),
            None,
            timing,
        ),
    );
}

fn log_metrics(
    app: &AppHandle,
    gateway: &InputGateway,
    context: MetricsLogContext,
    loop_timing: &mut LoopTimingWindow,
    final_window: bool,
) {
    let line = format!(
        "{} post_delay_ms={} nominal_period_ms={} elapsed_ms={} active={} session_attempts={} hold_cycles={} {}",
        gateway
            .metrics(InputSource::Spammer)
            .log_line(InputSource::Spammer, final_window),
        context.effective_delay_ms,
        context.timing.nominal_period_ms,
        context.elapsed.as_millis(),
        context.active,
        context.cycle_attempt,
        context.cycle_count,
        loop_timing.log_fields_and_reset(),
    );
    emit_tool_log_opt(Some(app), line);
}

fn next_cycle_deadline(completed_at: TokioInstant, delay: Duration) -> TokioInstant {
    completed_at + delay
}

fn duration_us(duration: Duration) -> u64 {
    duration.as_micros().min(u64::MAX as u128) as u64
}

fn timing_stats(values: &[u64]) -> (u64, u64, u64, u64) {
    if values.is_empty() {
        return (0, 0, 0, 0);
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let percentile = |percent: usize| {
        let index = ((sorted.len() - 1) * percent).div_ceil(100);
        sorted[index]
    };
    (
        percentile(50),
        percentile(95),
        percentile(99),
        *sorted.last().unwrap_or(&0),
    )
}

fn stall_flags(
    result: &CycleTaskResult,
    join_wait_us: u64,
    same_key_active: bool,
) -> Option<String> {
    let mut flags = Vec::new();
    match result.tick_result.as_ref() {
        Ok(tick) if !tick.cycled && same_key_active => flags.push("skipped"),
        Ok(tick) if !tick.cycled => return None,
        Err(_) => flags.push("error"),
        _ => {}
    }
    if result.timer_late_us >= TIMER_STALL_US {
        flags.push("timer");
    }
    if result.spawn_wait_us >= SPAWN_STALL_US {
        flags.push("spawn");
    }
    if result.task_us >= TASK_STALL_US {
        flags.push("task_or_input");
    }
    if join_wait_us >= JOIN_STALL_US {
        flags.push("join");
    }
    (!flags.is_empty()).then(|| flags.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ready_message() {
        let line = r#"{"type":"ready","devices":["AT keyboard"],"name":"AT keyboard","triggers":["F1","F2"]}"#;
        assert!(matches!(parse_line(line), Some(InputdMsg::Ready)));
    }

    #[test]
    fn parse_trigger_held_messages() {
        assert!(matches!(
            parse_line(r#"{"type":"trigger","key":"F2","held":true}"#),
            Some(InputdMsg::TriggerHeld { key, held: true }) if key == "F2"
        ));
        assert!(matches!(
            parse_line(r#"{"type":"trigger","key":"F2","held":false}"#),
            Some(InputdMsg::TriggerHeld { key, held: false }) if key == "F2"
        ));
    }

    #[test]
    fn parse_fatal_message() {
        match parse_line(r#"{"type":"fatal","message":"grab failed"}"#) {
            Some(InputdMsg::Fatal(msg)) => assert_eq!(msg, "grab failed"),
            other => panic!("expected fatal, got {other:?}"),
        }
    }

    #[test]
    fn parse_ignores_unknown_or_malformed() {
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"type":"shutdown"}"#).is_none());
        assert!(parse_line(r#"{"type":"trigger"}"#).is_none());
        assert!(parse_line(r#"{"type":"trigger","held":true}"#).is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn delayed_cycle_rearms_a_full_delay_without_catch_up() {
        let delay = Duration::from_millis(10);
        let cycle_sleep = sleep(Duration::ZERO);
        tokio::pin!(cycle_sleep);

        cycle_sleep
            .as_mut()
            .reset(next_cycle_deadline(TokioInstant::now(), delay));
        tokio::time::advance(delay).await;
        assert!(cycle_sleep.is_elapsed());

        tokio::time::advance(Duration::from_millis(50)).await;
        let delayed_completion = TokioInstant::now();
        cycle_sleep
            .as_mut()
            .reset(next_cycle_deadline(delayed_completion, delay));
        assert!(!cycle_sleep.is_elapsed());

        tokio::time::advance(delay - Duration::from_millis(1)).await;
        assert!(!cycle_sleep.is_elapsed());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(cycle_sleep.is_elapsed());
    }

    fn diagnostic_result(cycled: bool) -> CycleTaskResult {
        CycleTaskResult {
            attempt: 7,
            log_key: "F2".into(),
            tick_result: Ok(SpammerTick { cycled }),
            timer_late_us: 100,
            spawn_wait_us: 200,
            task_us: 31_000,
            finished_at: Instant::now(),
        }
    }

    #[test]
    fn loop_timing_window_keeps_maxima_and_resets() {
        let mut window = LoopTimingWindow::default();
        let mut first = diagnostic_result(true);
        window.record(&first, 300, true);
        first.timer_late_us = 50_000;
        first.spawn_wait_us = 4_000;
        first.task_us = 80_000;
        window.record(&first, 9_000, true);

        let line = window.log_fields_and_reset();
        assert!(line.contains("loop_samples=2"));
        assert!(line.contains("timer_late_us[p50/p95/p99/max]=50000/50000/50000/50000"));
        assert!(line.contains("task_us[p50/p95/p99/max]=80000/80000/80000/80000"));
        assert!(line.contains("completed=2"));

        let reset = window.log_fields_and_reset();
        assert!(reset.contains("loop_samples=0"));
        assert!(reset.contains("completed=0"));
    }

    #[test]
    fn stall_classifier_reports_combined_flags_and_ignores_expected_skip() {
        let normal = diagnostic_result(true);
        assert_eq!(stall_flags(&normal, 100, true), None);

        let mut timer = diagnostic_result(true);
        timer.timer_late_us = TIMER_STALL_US;
        assert_eq!(stall_flags(&timer, 100, true).as_deref(), Some("timer"));

        let mut task = diagnostic_result(true);
        task.spawn_wait_us = SPAWN_STALL_US;
        task.task_us = TASK_STALL_US;
        assert_eq!(
            stall_flags(&task, 100, true).as_deref(),
            Some("spawn+task_or_input")
        );

        let skipped = diagnostic_result(false);
        assert_eq!(stall_flags(&skipped, 100, true).as_deref(), Some("skipped"));
        assert_eq!(stall_flags(&skipped, 100, false), None);
    }

    #[test]
    fn inflight_watchdog_stage_tracks_spawn_task_and_join() {
        let stage = Arc::new(AtomicU8::new(INFLIGHT_WAITING_FOR_SPAWN));
        let inflight = InflightCycle {
            attempt: 1,
            key: "F2".into(),
            dispatched_at: Instant::now(),
            stage: Arc::clone(&stage),
        };
        assert_eq!(inflight.stage_label(), "spawn");

        stage.store(INFLIGHT_RUNNING_TASK, Ordering::Release);
        assert_eq!(inflight.stage_label(), "task_or_input");

        stage.store(INFLIGHT_FINISHED_TASK, Ordering::Release);
        assert_eq!(inflight.stage_label(), "join");
    }
}
