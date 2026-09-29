use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::{AutobuffConfig, ClientProfile, KeyPressWriter, MemoryReader, ToolsError};

const STATUS_SLOTS: usize = 100;
const QUAGMIRE: u32 = 8;
const OVERTHRUST: u32 = 25;
const OVERTHRUST_MAX: u32 = 188;
const QUAGMIRE_BLOCKED: [u32; 5] = [3, 23, 68, 115, 116];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutobuffTick {
    pub active_statuses: usize,
    pub applied_rule: Option<String>,
}

pub struct AutobuffEngine<M: MemoryReader, I: KeyPressWriter> {
    memory: M,
    input: I,
    config: AutobuffConfig,
    profile: ClientProfile,
    last_used: HashMap<String, Instant>,
}

impl<M: MemoryReader, I: KeyPressWriter> AutobuffEngine<M, I> {
    pub fn new(memory: M, input: I, config: AutobuffConfig, profile: ClientProfile) -> Self {
        Self {
            memory,
            input,
            config: config.clamped(),
            profile,
            last_used: HashMap::new(),
        }
    }

    pub fn update_config(&mut self, config: AutobuffConfig) {
        self.config = config.clamped();
    }

    pub fn replace_memory(&mut self, memory: M) {
        self.memory = memory;
        // Cooldowns belong to the old client. A replacement has no knowledge of
        // whether those key presses reached the new game process.
        self.last_used.clear();
    }

    pub fn tick(&mut self) -> Result<AutobuffTick, ToolsError> {
        let hp = self.memory.read_u32_slice(self.profile.hp_base, 2)?;
        let [current_hp, max_hp] = hp.as_slice() else {
            return Err(ToolsError::Other(format!(
                "AutoBuff: lectura HP incompleta ({} de 2)",
                hp.len()
            )));
        };
        // The status buffer can be readable but empty before character login.
        // Do not spend a rule's cooldown on an input the game cannot use yet.
        if *current_hp == 0 || *max_hp == 0 || current_hp > max_hp {
            return Ok(AutobuffTick::default());
        }
        let statuses = self
            .memory
            .read_u32_slice(self.profile.status_buffer_address(), STATUS_SLOTS)?;
        let active: HashSet<u32> = statuses
            .into_iter()
            .filter(|id| *id != 0 && *id != u32::MAX && *id != 210_803_216)
            .collect();
        let quagmire = active.contains(&QUAGMIRE);
        let now = Instant::now();
        let mut rules: Vec<_> = self
            .config
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .collect();
        rules.sort_by_key(|rule| rule.priority);

        for rule in rules {
            let present = active.contains(&rule.status_id)
                || (rule.status_id == OVERTHRUST && active.contains(&OVERTHRUST_MAX));
            let cooling_down = self.last_used.get(&rule.id).is_some_and(|last| {
                now.duration_since(*last) < Duration::from_millis(rule.cooldown_ms)
            });
            if present || cooling_down || (quagmire && QUAGMIRE_BLOCKED.contains(&rule.status_id)) {
                continue;
            }
            self.input.press_key(&rule.key)?;
            self.last_used.insert(rule.id.clone(), now);
            return Ok(AutobuffTick {
                active_statuses: active.len(),
                applied_rule: Some(rule.label.clone()),
            });
        }
        Ok(AutobuffTick {
            active_statuses: active.len(),
            applied_rule: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AutobuffRule;
    use std::sync::Mutex;
    struct Memory {
        hp: u32,
        statuses: Vec<u32>,
    }
    fn memory(hp: u32, status: u32) -> Memory {
        Memory {
            hp,
            statuses: vec![status; STATUS_SLOTS],
        }
    }
    impl MemoryReader for Memory {
        fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
            Ok(match address {
                0x1000 => self.hp,
                0x1004 => 100,
                _ => self.statuses[((address - 0x1474) / 4) as usize],
            })
        }
        fn read_string(&self, _: u32, _: usize) -> Result<String, ToolsError> {
            Ok(String::new())
        }
        fn read_u32_slice(&self, address: u32, _: usize) -> Result<Vec<u32>, ToolsError> {
            match address {
                0x1000 => Ok(vec![self.hp, 100]),
                0x1474 => Ok(self.statuses.clone()),
                _ => Err(ToolsError::Other("unexpected address".into())),
            }
        }
    }
    struct Input(Mutex<Vec<String>>);
    impl KeyPressWriter for Input {
        fn press_key(&self, key: &str) -> Result<(), ToolsError> {
            self.0.lock().unwrap().push(key.into());
            Ok(())
        }
    }
    fn profile() -> ClientProfile {
        ClientProfile {
            id: "test".into(),
            label: "test".into(),
            exe_names: vec![],
            hp_base: 0x1000,
            name_address: 0,
        }
    }
    fn rule(status_id: u32) -> AutobuffRule {
        AutobuffRule {
            id: "agi".into(),
            label: "AGI".into(),
            status_id,
            key: "F1".into(),
            cooldown_ms: 100,
            priority: 0,
            enabled: true,
        }
    }
    #[test]
    fn casts_only_when_missing() {
        let input = Input(Mutex::new(vec![]));
        let mut engine = AutobuffEngine::new(
            memory(100, 3),
            input,
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule(3)],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap().applied_rule, None);
    }
    #[test]
    fn treats_overthrust_max_as_overthrust() {
        let input = Input(Mutex::new(vec![]));
        let mut engine = AutobuffEngine::new(
            memory(100, OVERTHRUST_MAX),
            input,
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule(OVERTHRUST)],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap().applied_rule, None);
    }

    #[test]
    fn casts_the_highest_priority_missing_rule() {
        let input = Input(Mutex::new(vec![]));
        let mut engine = AutobuffEngine::new(
            memory(100, 0),
            input,
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule(3)],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap().applied_rule.as_deref(), Some("AGI"));
    }

    #[test]
    fn replace_memory_switches_status_reads() {
        let input = Input(Mutex::new(vec![]));
        let mut engine = AutobuffEngine::new(
            memory(100, 0),
            input,
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule(3)],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap().applied_rule.as_deref(), Some("AGI"));
        engine.replace_memory(memory(100, 3));
        assert_eq!(engine.tick().unwrap().applied_rule, None);
    }

    #[test]
    fn new_client_can_apply_a_rule_used_by_the_previous_client() {
        let mut rule = rule(3);
        rule.cooldown_ms = 60_000;
        let mut engine = AutobuffEngine::new(
            memory(100, 0),
            Input(Mutex::new(vec![])),
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap().applied_rule.as_deref(), Some("AGI"));
        assert_eq!(engine.tick().unwrap().applied_rule, None);

        engine.replace_memory(memory(100, 0));
        assert_eq!(engine.tick().unwrap().applied_rule.as_deref(), Some("AGI"));
    }

    #[test]
    fn empty_status_buffer_during_login_does_not_use_a_cooldown() {
        let mut engine = AutobuffEngine::new(
            memory(0, 0),
            Input(Mutex::new(vec![])),
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule(3)],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap(), AutobuffTick::default());
        assert!(engine.input.0.lock().unwrap().is_empty());

        engine.memory.hp = 100;
        assert_eq!(engine.tick().unwrap().applied_rule.as_deref(), Some("AGI"));
    }

    #[test]
    fn quagmire_blocks_incompatible_buffs() {
        let input = Input(Mutex::new(vec![]));
        let mut engine = AutobuffEngine::new(
            memory(100, QUAGMIRE),
            input,
            AutobuffConfig {
                enabled: true,
                delay_ms: 300,
                rules: vec![rule(3)],
            },
            profile(),
        );
        assert_eq!(engine.tick().unwrap().applied_rule, None);
    }
}
