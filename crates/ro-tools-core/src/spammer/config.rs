use serde::{Deserialize, Serialize};

use crate::error::ToolsError;
use crate::spammer::keys::normalize_spammer_keys;

fn default_spammer_delay_ms() -> u64 {
    10
}

fn default_spammer_keys() -> Vec<String> {
    vec!["F1".into()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpammerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_spammer_delay_ms")]
    pub delay_ms: u64,
    #[serde(default = "default_spammer_keys")]
    pub keys: Vec<String>,
}

impl Default for SpammerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: default_spammer_delay_ms(),
            keys: default_spammer_keys(),
        }
    }
}

impl SpammerConfig {
    pub fn clamped(&self) -> Self {
        let mut c = self.normalized();
        c.delay_ms = c.delay_ms.clamp(5, 100);
        c
    }

    pub fn normalized(&self) -> Self {
        let mut c = self.clone();
        c.keys = normalize_spammer_keys(&self.keys);
        c
    }

    pub fn validate_for_start(&self) -> Result<(), ToolsError> {
        let c = self.normalized();
        if c.enabled && c.keys.is_empty() {
            return Err(ToolsError::Other(
                "Spammer: selecciona al menos una tecla".into(),
            ));
        }
        Ok(())
    }
}
