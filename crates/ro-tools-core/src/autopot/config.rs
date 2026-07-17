use serde::{Deserialize, Serialize};

use crate::{parse_hex, ToolsError};

fn default_hp_key() -> String {
    "F8".into()
}

fn default_sp_key() -> String {
    "F9".into()
}

fn default_hp_percent() -> u32 {
    80
}

fn default_sp_percent() -> u32 {
    50
}

fn default_delay_ms() -> u64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutopotConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_hp_key")]
    pub hp_key: String,
    #[serde(default = "default_sp_key")]
    pub sp_key: String,
    #[serde(default = "default_hp_percent")]
    pub hp_percent: u32,
    #[serde(default = "default_sp_percent")]
    pub sp_percent: u32,
    #[serde(default = "default_delay_ms")]
    pub delay_ms: u64,
    /// Sends an HP key press on idle AutoPot cycles to reduce reaction time on high-latency servers.
    #[serde(default)]
    pub proactive_mode: bool,
    /// Optional override of profile id from registry
    #[serde(default)]
    pub profile_id: Option<String>,
    /// Optional manual HP base override (hex string, e.g. "0x10DCE10")
    #[serde(default)]
    pub hp_base_override: Option<String>,
}

impl Default for AutopotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hp_key: default_hp_key(),
            sp_key: default_sp_key(),
            hp_percent: default_hp_percent(),
            sp_percent: default_sp_percent(),
            delay_ms: default_delay_ms(),
            proactive_mode: false,
            profile_id: None,
            hp_base_override: None,
        }
    }
}

impl AutopotConfig {
    pub fn clamped(&self) -> Self {
        let mut c = self.clone();
        c.hp_percent = c.hp_percent.clamp(1, 99);
        c.sp_percent = c.sp_percent.clamp(1, 99);
        c.delay_ms = c.delay_ms.clamp(10, 2000);
        c
    }

    pub fn validate(&self) -> Result<(), ToolsError> {
        if self.hp_key.trim().is_empty() || self.sp_key.trim().is_empty() {
            return Err(ToolsError::Other(
                "AutoPot: las teclas de HP y SP no pueden estar vacías".into(),
            ));
        }
        if self
            .profile_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(ToolsError::Other(
                "AutoPot: el perfil no puede estar vacío".into(),
            ));
        }
        if let Some(address) = &self.hp_base_override {
            parse_hex(address).map_err(ToolsError::Other)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_shared_default_fixture() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../contract-fixtures/server-configs.json"
        ))
        .unwrap();
        let expected: AutopotConfig =
            serde_json::from_value(fixtures["defaults"]["autopot"].clone()).unwrap();
        assert_eq!(AutopotConfig::default(), expected);
    }
}
