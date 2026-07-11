use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{spammer::keys::is_valid_spammer_key, ToolsError};

fn default_delay_ms() -> u64 {
    300
}
fn default_rule_cooldown_ms() -> u64 {
    1_000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutobuffRule {
    pub id: String,
    pub label: String,
    pub status_id: u32,
    pub key: String,
    #[serde(default = "default_rule_cooldown_ms")]
    pub cooldown_ms: u64,
    #[serde(default)]
    pub priority: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutobuffConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_delay_ms")]
    pub delay_ms: u64,
    #[serde(default)]
    pub rules: Vec<AutobuffRule>,
}

impl Default for AutobuffConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: default_delay_ms(),
            rules: vec![],
        }
    }
}

impl AutobuffConfig {
    pub fn clamped(&self) -> Self {
        let mut config = self.normalized();
        config.delay_ms = config.delay_ms.clamp(100, 2_000);
        for rule in &mut config.rules {
            rule.cooldown_ms = rule.cooldown_ms.clamp(100, 60_000);
        }
        config
    }

    pub fn normalized(&self) -> Self {
        let mut config = self.clone();
        for rule in &mut config.rules {
            rule.key = rule.key.trim().to_ascii_uppercase();
        }
        config
    }

    pub fn validate(&self) -> Result<(), ToolsError> {
        let config = self.normalized();
        let mut status_ids = HashSet::new();
        for rule in &config.rules {
            if rule.id.trim().is_empty()
                || rule.label.trim().is_empty()
                || rule.key.trim().is_empty()
            {
                return Err(ToolsError::Other(
                    "AutoBuff: cada regla necesita identificador, nombre y tecla".into(),
                ));
            }
            if rule.status_id == 0 {
                return Err(ToolsError::Other(
                    "AutoBuff: el status ID debe ser mayor que cero".into(),
                ));
            }
            if !is_valid_spammer_key(&rule.key) {
                return Err(ToolsError::Other(format!(
                    "AutoBuff: tecla no soportada para '{}': {}",
                    rule.label, rule.key
                )));
            }
            if !status_ids.insert(rule.status_id) {
                return Err(ToolsError::Other(format!(
                    "AutoBuff: el status ID {} está repetido",
                    rule.status_id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, status_id: u32) -> AutobuffRule {
        AutobuffRule {
            id: id.into(),
            label: id.into(),
            status_id,
            key: "F1".into(),
            cooldown_ms: 1_000,
            priority: 0,
            enabled: true,
        }
    }

    #[test]
    fn rejects_duplicate_status_ids() {
        let config = AutobuffConfig {
            rules: vec![rule("first", 12), rule("second", 12)],
            ..Default::default()
        };
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("repetido"));
    }

    #[test]
    fn rejects_zero_status_id() {
        let config = AutobuffConfig {
            rules: vec![rule("invalid", 0)],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn normalizes_lowercase_keys_and_rejects_unsupported_ones() {
        let mut config = AutobuffConfig {
            rules: vec![rule("valid", 12)],
            ..Default::default()
        };
        config.rules[0].key = "f8".into();
        assert_eq!(config.clamped().rules[0].key, "F8");
        assert!(config.validate().is_ok());

        config.rules[0].key = "F13".into();
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("no soportada"));
    }
}
