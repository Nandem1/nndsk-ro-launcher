use std::time::Duration;

use ro_tools_core::{HeldKeyWriter, KeyPressWriter, SpamCycleWriter, ToolsError};

use super::uinput_worker::{InputSource, MetricsSnapshot, UinputInput, UinputWriter};

/// Shared gateway to the single persistent, prioritized uinput worker.
#[derive(Clone)]
pub struct InputGateway {
    uinput: UinputInput,
}

impl InputGateway {
    pub fn new() -> Self {
        Self {
            uinput: UinputInput::new(),
        }
    }

    pub fn writer(
        &self,
        source: InputSource,
        effective_delay_ms: u64,
    ) -> Result<GatewayWriter, ToolsError> {
        self.uinput
            .writer(source, Duration::from_millis(effective_delay_ms.max(10)))
    }

    pub async fn prepare(&self) -> Result<String, ToolsError> {
        let uinput = self.uinput.clone();
        tokio::task::spawn_blocking(move || uinput.prepare())
            .await
            .map_err(|error| {
                ToolsError::Other(format!(
                    "uinput stage=join preparation device=both errno=none: {error}"
                ))
            })?
    }

    pub fn is_prepared(&self) -> bool {
        self.uinput.is_prepared()
    }

    pub fn metrics(&self, source: InputSource) -> MetricsSnapshot {
        self.uinput.snapshot_metrics(source)
    }

    pub fn shutdown(&self) {
        self.uinput.shutdown();
    }
}

impl Default for InputGateway {
    fn default() -> Self {
        Self::new()
    }
}

pub type GatewayWriter = UinputWriter;

impl KeyPressWriter for UinputWriter {
    fn press_key(&self, key: &str) -> Result<(), ToolsError> {
        UinputWriter::press_key(self, key)
    }
}

impl HeldKeyWriter for UinputWriter {
    fn key_down(&self, key: &str) -> Result<(), ToolsError> {
        self.key_event(key, 1)
    }

    fn key_up(&self, key: &str) -> Result<(), ToolsError> {
        self.key_event(key, 0)
    }
}

impl SpamCycleWriter for UinputWriter {
    fn spam_cycle(
        &self,
        key: &str,
        deadline: Option<std::time::Instant>,
    ) -> Result<bool, ToolsError> {
        UinputWriter::spam_cycle(self, key, deadline)
    }
}
