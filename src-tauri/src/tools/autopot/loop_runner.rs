use std::sync::{Arc, Mutex};
use std::time::Instant;

use ro_tools_core::{AutopotConfig, AutopotEngine};
use ro_tools_linux::LazyYdotoolInput;
use tauri::AppHandle;
use tokio::sync::watch;

use crate::models::autopot::AutopotStatusEvent;
use crate::tools::input::YdotoolDaemon;

use super::service::{
    emit_status_if_changed, new_ticker, recover_ydotool_on_error, SharedYdotoolInput,
};

pub async fn run(
    app: AppHandle,
    memory: ro_tools_linux::ProcMemoryReader,
    input: Arc<LazyYdotoolInput>,
    config: AutopotConfig,
    profile: ro_tools_core::ClientProfile,
    mut stop_rx: watch::Receiver<bool>,
    mut config_rx: watch::Receiver<AutopotConfig>,
    status_arc: Arc<Mutex<AutopotStatusEvent>>,
    ydotoold: Arc<YdotoolDaemon>,
) {
    let engine = Arc::new(Mutex::new(AutopotEngine::new(
        memory,
        SharedYdotoolInput(Arc::clone(&input)),
        config.clone(),
        profile,
    )));
    let mut current_config = config;
    let mut ticker = new_ticker(current_config.delay_ms);
    let mut tick_count: u64 = 0;
    let mut last_ydotool_recovery = Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or_else(Instant::now);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                tick_count += 1;
                let engine = Arc::clone(&engine);
                let tick_result = tokio::task::spawn_blocking(move || {
                    engine.lock().unwrap().tick()
                }).await;

                match tick_result {
                    Ok(Ok(tick)) => {
                        if tick.potted_hp || tick.potted_sp {
                            crate::utils::emit_tool_log_opt(
                                Some(&app),
                                format!(
                                    "[AutoPot] tick#{tick_count} pot HP={} SP={} | {}/{} HP · {}/{} SP · '{}'",
                                    if tick.potted_hp { "sí" } else { "—" },
                                    if tick.potted_sp { "sí" } else { "—" },
                                    tick.cur_hp, tick.max_hp,
                                    tick.cur_sp, tick.max_sp,
                                    tick.character_name,
                                ),
                            );
                        }
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            AutopotStatusEvent {
                                active: true,
                                cur_hp: tick.cur_hp,
                                max_hp: tick.max_hp,
                                cur_sp: tick.cur_sp,
                                max_sp: tick.max_sp,
                                hp_percent: current_config.hp_percent,
                                sp_percent: current_config.sp_percent,
                                character_name: tick.character_name,
                                error: None,
                            },
                        );
                    }
                    Ok(Err(e)) => {
                        let err_msg = e.to_string();
                        recover_ydotool_on_error(
                            &app,
                            input.as_ref(),
                            ydotoold.as_ref(),
                            &mut last_ydotool_recovery,
                            &err_msg,
                        )
                        .await;

                        let prev = status_arc.lock().unwrap().clone();
                        emit_status_if_changed(
                            &app,
                            &status_arc,
                            AutopotStatusEvent {
                                active: true,
                                cur_hp: prev.cur_hp,
                                max_hp: prev.max_hp,
                                cur_sp: prev.cur_sp,
                                max_sp: prev.max_sp,
                                character_name: prev.character_name,
                                error: Some(err_msg.clone()),
                                hp_percent: current_config.hp_percent,
                                sp_percent: current_config.sp_percent,
                            },
                        );
                        crate::utils::emit_tool_log_opt(
                            Some(&app),
                            format!("[AutoPot] ERROR tick: {err_msg}"),
                        );
                    }
                    Err(e) => {
                        crate::utils::emit_tool_log_opt(
                            Some(&app),
                            format!("[AutoPot] ERROR tick (join): {e}"),
                        );
                    }
                }
            }
            changed = config_rx.changed() => {
                if changed.is_ok() {
                    current_config = config_rx.borrow().clone();
                    engine.lock().unwrap().update_config(current_config.clone());
                    ticker = new_ticker(current_config.delay_ms);
                    crate::utils::emit_tool_log_opt(
                        Some(&app),
                        format!(
                            "[AutoPot] Config actualizada HP={}% SP={}% delay={}ms",
                            current_config.hp_percent,
                            current_config.sp_percent,
                            current_config.delay_ms,
                        ),
                    );
                }
            }
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() {
                    break;
                }
            }
        }
    }

    crate::utils::emit_tool_log_opt(Some(&app), "[AutoPot] Loop detenido");
    let idle = AutopotStatusEvent {
        active: false,
        hp_percent: current_config.hp_percent,
        sp_percent: current_config.sp_percent,
        ..AutopotStatusEvent::default()
    };
    emit_status_if_changed(&app, &status_arc, idle);
}
