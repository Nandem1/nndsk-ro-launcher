use ro_tools_core::SpammerConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpammerStatusEvent {
    pub active: bool,
    pub armed: bool,
    pub spamming: bool,
    pub key: String,
    pub delay_ms: u64,
    pub cycle_count: u64,
    pub error: Option<String>,
}

impl Default for SpammerStatusEvent {
    fn default() -> Self {
        Self {
            active: false,
            armed: false,
            spamming: false,
            key: String::new(),
            delay_ms: SpammerConfig::default().delay_ms,
            cycle_count: 0,
            error: None,
        }
    }
}
