use ro_tools_core::SpammerConfig;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::models::spammer::SpammerStatusEvent;
use crate::tools::input::InputGateway;
use crate::tools::session::SessionController;
use crate::utils::emit_tool_log_opt;

pub struct SpammerHandle {
    session: SessionController,
    status: Arc<Mutex<SpammerStatusEvent>>,
}

impl Clone for SpammerHandle {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            status: Arc::clone(&self.status),
        }
    }
}

impl SpammerHandle {
    pub fn new() -> Self {
        Self {
            session: SessionController::new("Spammer"),
            status: Arc::new(Mutex::new(SpammerStatusEvent::default())),
        }
    }

    pub fn status(&self) -> SpammerStatusEvent {
        self.status.lock().unwrap().clone()
    }

    pub async fn stop(&self) -> Result<(), String> {
        let result = self.session.stop().await;
        let mut status = self.status.lock().unwrap();
        status.active = false;
        status.armed = false;
        status.spamming = false;
        result
    }

    pub async fn start(
        &self,
        app: AppHandle,
        input: InputGateway,
        config: SpammerConfig,
    ) -> Result<(), String> {
        let mut config = config.clamped();
        config.enabled = true;
        config.validate_for_start().map_err(|e| e.to_string())?;
        let status_arc = Arc::clone(&self.status);
        let timing = super::timing::SpammerTimingPlan::new(config.delay_ms);
        let effective_delay_ms = timing.post_delay_ms;
        let writer = input
            .spammer_writer(timing.cycle, timing.deadline_budget_ms)
            .map_err(|error| error.to_string())?;

        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Spammer] Standby {} + click backend=uinput pausa post-ciclo={}ms — mantén la tecla en el juego",
                config.keys.join(","),
                effective_delay_ms,
            ),
        );
        emit_tool_log_opt(
            Some(&app),
            format!("[Spammer][diag] timing {}", timing.log_line()),
        );

        self.session
            .replace(move |stop_rx| async move {
                super::loop_runner::run(app, writer, config, timing, stop_rx, status_arc, input)
                    .await;
            })
            .await?;
        Ok(())
    }
}
