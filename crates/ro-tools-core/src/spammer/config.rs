use serde::{Deserialize, Serialize};

use crate::error::ToolsError;
use crate::spammer::keys::{is_valid_spammer_key, normalize_spammer_keys};

fn default_spammer_delay_ms() -> u64 {
    10
}

fn default_spammer_keys() -> Vec<String> {
    vec!["F1".into()]
}

fn default_switch_delay_ms() -> u64 {
    50
}

/// Una regla de cambio de equipo: al presionar `trigger` se equipan `atk_keys`,
/// al soltarlo se equipan `def_keys`. `trigger` debe ser una tecla del spammer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GearSwitchRule {
    pub trigger: String,
    #[serde(default)]
    pub atk_keys: Vec<String>,
    #[serde(default)]
    pub def_keys: Vec<String>,
}

impl GearSwitchRule {
    fn clamped(&self) -> Option<Self> {
        let trigger = self.trigger.trim().to_ascii_uppercase();
        if !is_valid_spammer_key(&trigger) {
            return None;
        }
        Some(Self {
            trigger,
            atk_keys: normalize_spammer_keys(&self.atk_keys),
            def_keys: normalize_spammer_keys(&self.def_keys),
        })
    }
}

/// Cambio de equipo ATK/DEF acoplado al spammer. Cada regla mapea un trigger
/// (tecla del spammer) a un set ATK (al presionar) y un set DEF (al soltar).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GearSwitchConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_switch_delay_ms")]
    pub switch_delay_ms: u64,
    #[serde(default)]
    pub rules: Vec<GearSwitchRule>,
}

impl Default for GearSwitchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            switch_delay_ms: default_switch_delay_ms(),
            rules: Vec::new(),
        }
    }
}

impl GearSwitchConfig {
    fn clamped(&self) -> Self {
        let mut seen = std::collections::HashSet::new();
        let rules = self
            .rules
            .iter()
            .filter_map(GearSwitchRule::clamped)
            // Un trigger no puede repetirse entre reglas: gana la primera.
            .filter(|rule| seen.insert(rule.trigger.clone()))
            .collect();
        Self {
            enabled: self.enabled,
            switch_delay_ms: self.switch_delay_ms.clamp(10, 300),
            rules,
        }
    }

    /// Devuelve la regla cuyo trigger coincide con `key`, si existe.
    pub fn rule_for(&self, key: &str) -> Option<&GearSwitchRule> {
        self.rules.iter().find(|rule| rule.trigger == key)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpammerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_spammer_delay_ms")]
    pub delay_ms: u64,
    #[serde(default = "default_spammer_keys")]
    pub keys: Vec<String>,
    #[serde(default)]
    pub gear_switch: GearSwitchConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpammerConfigWire {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_spammer_delay_ms")]
    delay_ms: u64,
    #[serde(default = "default_spammer_keys")]
    keys: Vec<String>,
    #[serde(default)]
    gear_switch: GearSwitchConfigWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GearSwitchConfigWire {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_switch_delay_ms")]
    switch_delay_ms: u64,
    #[serde(default)]
    rules: Vec<GearSwitchRule>,
    // Esquema global anterior. Se conserva sólo para migrar servers.json.
    #[serde(default)]
    trigger_keys: Vec<String>,
    #[serde(default)]
    atk_keys: Vec<String>,
    #[serde(default)]
    def_keys: Vec<String>,
}

impl Default for GearSwitchConfigWire {
    fn default() -> Self {
        Self {
            enabled: false,
            switch_delay_ms: default_switch_delay_ms(),
            rules: Vec::new(),
            trigger_keys: Vec::new(),
            atk_keys: Vec::new(),
            def_keys: Vec::new(),
        }
    }
}

impl GearSwitchConfigWire {
    fn into_config(self, spammer_keys: &[String]) -> GearSwitchConfig {
        let has_legacy_config =
            !self.trigger_keys.is_empty() || !self.atk_keys.is_empty() || !self.def_keys.is_empty();
        let rules = if self.rules.is_empty() && has_legacy_config {
            let triggers = if self.trigger_keys.is_empty() {
                spammer_keys.to_vec()
            } else {
                self.trigger_keys
            };
            triggers
                .into_iter()
                .map(|trigger| GearSwitchRule {
                    trigger,
                    atk_keys: self.atk_keys.clone(),
                    def_keys: self.def_keys.clone(),
                })
                .collect()
        } else {
            self.rules
        };

        GearSwitchConfig {
            enabled: self.enabled,
            switch_delay_ms: self.switch_delay_ms,
            rules,
        }
    }
}

impl<'de> Deserialize<'de> for SpammerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SpammerConfigWire::deserialize(deserializer)?;
        let gear_switch = wire.gear_switch.into_config(&wire.keys);
        Ok(Self {
            enabled: wire.enabled,
            delay_ms: wire.delay_ms,
            keys: wire.keys,
            gear_switch,
        })
    }
}

impl Default for SpammerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay_ms: default_spammer_delay_ms(),
            keys: default_spammer_keys(),
            gear_switch: GearSwitchConfig::default(),
        }
    }
}

impl SpammerConfig {
    pub fn clamped(&self) -> Self {
        let mut c = self.normalized();
        c.delay_ms = c.delay_ms.clamp(10, 100);
        c
    }

    pub fn normalized(&self) -> Self {
        let mut c = self.clone();
        c.keys = normalize_spammer_keys(&self.keys);
        c.gear_switch = self.gear_switch.clamped();
        c.gear_switch
            .rules
            .retain(|rule| c.keys.contains(&rule.trigger));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_switch_defaults_when_absent() {
        let cfg: SpammerConfig = serde_json::from_str(r#"{"keys":["F3"]}"#).unwrap();
        assert!(!cfg.gear_switch.enabled);
        assert_eq!(cfg.gear_switch.switch_delay_ms, 50);
        assert!(cfg.gear_switch.rules.is_empty());
    }

    #[test]
    fn clamped_normalizes_rules_and_delay() {
        let cfg = SpammerConfig {
            enabled: true,
            delay_ms: 10,
            keys: vec!["f3".into()],
            gear_switch: GearSwitchConfig {
                enabled: true,
                switch_delay_ms: 5000,
                rules: vec![GearSwitchRule {
                    trigger: "f3".into(),
                    atk_keys: vec!["8".into(), "bogus".into()],
                    def_keys: vec!["9".into()],
                }],
            },
        };
        let c = cfg.clamped();
        assert_eq!(c.gear_switch.switch_delay_ms, 300);
        assert_eq!(c.gear_switch.rules.len(), 1);
        assert_eq!(c.gear_switch.rules[0].trigger, "F3");
        assert_eq!(c.gear_switch.rules[0].atk_keys, vec!["8"]);
    }

    #[test]
    fn clamped_drops_invalid_and_duplicate_triggers() {
        let gear = GearSwitchConfig {
            enabled: true,
            switch_delay_ms: 50,
            rules: vec![
                GearSwitchRule {
                    trigger: "F3".into(),
                    atk_keys: vec!["8".into()],
                    def_keys: vec!["9".into()],
                },
                GearSwitchRule {
                    trigger: "f3".into(), // duplicado
                    atk_keys: vec!["1".into()],
                    def_keys: vec![],
                },
                GearSwitchRule {
                    trigger: "space".into(), // inválido
                    atk_keys: vec![],
                    def_keys: vec![],
                },
            ],
        };
        let cfg = SpammerConfig {
            enabled: true,
            delay_ms: 10,
            keys: vec!["F3".into()],
            gear_switch: gear,
        };
        let c = cfg.clamped();
        assert_eq!(c.gear_switch.rules.len(), 1);
        assert_eq!(c.gear_switch.rules[0].atk_keys, vec!["8"]);
    }

    #[test]
    fn rule_for_finds_by_trigger() {
        let gear = GearSwitchConfig {
            enabled: true,
            switch_delay_ms: 50,
            rules: vec![GearSwitchRule {
                trigger: "F3".into(),
                atk_keys: vec!["8".into()],
                def_keys: vec!["9".into()],
            }],
        };
        assert!(gear.rule_for("F3").is_some());
        assert!(gear.rule_for("F1").is_none());
    }

    #[test]
    fn deserializes_independent_rules() {
        let cfg: SpammerConfig = serde_json::from_str(
            r#"{
                "keys":["F3","F4"],
                "gearSwitch":{
                    "enabled":true,
                    "switchDelayMs":60,
                    "rules":[
                        {"trigger":"F3","atkKeys":["8"],"defKeys":["9"]},
                        {"trigger":"F4","atkKeys":["1"],"defKeys":["2"]}
                    ]
                }
            }"#,
        )
        .unwrap();
        let cfg = cfg.normalized();
        assert_eq!(cfg.gear_switch.rules.len(), 2);
        assert_eq!(cfg.gear_switch.rules[1].trigger, "F4");
        assert_eq!(cfg.gear_switch.rules[1].atk_keys, vec!["1"]);
    }

    #[test]
    fn migrates_legacy_shared_gear_to_each_trigger() {
        let cfg: SpammerConfig = serde_json::from_str(
            r#"{
                "keys":["F3","F4"],
                "gearSwitch":{
                    "enabled":true,
                    "switchDelayMs":60,
                    "triggerKeys":[],
                    "atkKeys":["8"],
                    "defKeys":["9"]
                }
            }"#,
        )
        .unwrap();
        let cfg = cfg.normalized();
        assert_eq!(cfg.gear_switch.rules.len(), 2);
        assert_eq!(cfg.gear_switch.rules[0].trigger, "F3");
        assert_eq!(cfg.gear_switch.rules[1].trigger, "F4");
        assert_eq!(cfg.gear_switch.rules[1].def_keys, vec!["9"]);
    }

    #[test]
    fn normalized_drops_rules_without_an_active_spammer_trigger() {
        let cfg = SpammerConfig {
            keys: vec!["F4".into()],
            gear_switch: GearSwitchConfig {
                enabled: true,
                switch_delay_ms: 50,
                rules: vec![
                    GearSwitchRule {
                        trigger: "F3".into(),
                        atk_keys: vec!["8".into()],
                        def_keys: vec!["9".into()],
                    },
                    GearSwitchRule {
                        trigger: "F4".into(),
                        atk_keys: vec!["1".into()],
                        def_keys: vec!["2".into()],
                    },
                ],
            },
            ..Default::default()
        }
        .normalized();
        assert_eq!(cfg.gear_switch.rules.len(), 1);
        assert_eq!(cfg.gear_switch.rules[0].trigger, "F4");
    }

    #[test]
    fn matches_shared_legacy_migration_fixture() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../contract-fixtures/server-configs.json"
        ))
        .unwrap();
        let config: SpammerConfig =
            serde_json::from_value(fixtures["legacySpammer"]["input"].clone()).unwrap();
        let actual = serde_json::to_value(config).unwrap();
        assert_eq!(actual, fixtures["legacySpammer"]["expectedCanonical"]);
    }

    #[test]
    fn matches_shared_default_fixture() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../contract-fixtures/server-configs.json"
        ))
        .unwrap();
        let actual = serde_json::to_value(SpammerConfig::default()).unwrap();
        assert_eq!(actual, fixtures["defaults"]["spammer"]);
    }
}
