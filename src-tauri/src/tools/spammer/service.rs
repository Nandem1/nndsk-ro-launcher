use ro_tools_core::SpammerConfig;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::models::spammer::SpammerStatusEvent;
use crate::tools::input::{InputGateway, YdotoolDaemon};
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
        ydotoold: Arc<YdotoolDaemon>,
    ) -> Result<(), String> {
        let mut config = config.clamped();
        config.enabled = true;
        config.validate_for_start().map_err(|e| e.to_string())?;
        let status_arc = Arc::clone(&self.status);
        let writer = input.writer();

        emit_tool_log_opt(
            Some(&app),
            format!(
                "[Spammer] Standby {} + click delay={}ms — mantén la tecla en el juego",
                config.keys.join(","),
                config.delay_ms
            ),
        );

        self.session
            .replace(move |stop_rx| async move {
                super::loop_runner::run(app, writer, config, stop_rx, status_arc, input, ydotoold)
                    .await;
            })
            .await?;
        Ok(())
    }
}
