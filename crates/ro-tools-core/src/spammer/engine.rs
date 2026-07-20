use crate::error::ToolsError;
use crate::ports::SpamCycleWriter;
use crate::spammer::config::SpammerConfig;
use crate::spammer::keys::is_valid_spammer_key;
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpammerTick {
    pub cycled: bool,
}

pub struct SpammerEngine<I: SpamCycleWriter> {
    input: I,
    config: SpammerConfig,
}

impl<I: SpamCycleWriter> SpammerEngine<I> {
    pub fn new(input: I, config: SpammerConfig) -> Self {
        Self {
            input,
            config: config.clamped(),
        }
    }

    pub fn update_config(&mut self, config: SpammerConfig) {
        self.config = config.clamped();
    }

    pub fn config(&self) -> &SpammerConfig {
        &self.config
    }

    /// Ciclo IPC-mode atómico con una activación fresca y exactamente un click.
    /// El backend conserva/rearma la tecla; `release` cierra el estado al detenerse.
    pub fn tick(&mut self, key: &str) -> Result<SpammerTick, ToolsError> {
        self.tick_with_deadline(key, None)
    }

    pub fn release(&mut self) -> Result<(), ToolsError> {
        self.input.release_spam()
    }

    pub fn tick_with_deadline(
        &mut self,
        key: &str,
        deadline: Option<Instant>,
    ) -> Result<SpammerTick, ToolsError> {
        let key = key.trim();
        if !is_valid_spammer_key(key) {
            return Err(ToolsError::Input {
                key: key.to_string(),
                message: "tecla spammer no soportada".into(),
            });
        }

        let cycled = self.input.spam_cycle(key, deadline)?;

        Ok(SpammerTick { cycled })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::SpamCycleWriter;
    use std::sync::Mutex;

    struct MockInput {
        log: Mutex<Vec<String>>,
    }

    impl SpamCycleWriter for MockInput {
        fn spam_cycle(&self, key: &str, _deadline: Option<Instant>) -> Result<bool, ToolsError> {
            self.log.lock().unwrap().push(format!("cycle:{key}"));
            Ok(true)
        }

        fn release_spam(&self) -> Result<(), ToolsError> {
            self.log.lock().unwrap().push("release".into());
            Ok(())
        }
    }

    #[test]
    fn spammer_key_and_click() {
        let input = MockInput {
            log: Mutex::new(vec![]),
        };
        let mut engine = SpammerEngine::new(
            input,
            SpammerConfig {
                enabled: true,
                delay_ms: 10,
                keys: vec!["F2".into()],
                gear_switch: Default::default(),
            },
        );

        let tick = engine.tick("F2").unwrap();
        assert!(tick.cycled);
        engine.release().unwrap();

        let log = engine.input.log.lock().unwrap();
        assert_eq!(log.as_slice(), &["cycle:F2", "release"]);
    }

    #[test]
    fn spammer_rejects_invalid_key() {
        let input = MockInput {
            log: Mutex::new(vec![]),
        };
        let mut engine = SpammerEngine::new(input, SpammerConfig::default());
        assert!(engine.tick("SPACE").is_err());
    }

    #[test]
    fn spammer_accepts_letter_key() {
        let input = MockInput {
            log: Mutex::new(vec![]),
        };
        let mut engine = SpammerEngine::new(input, SpammerConfig::default());

        assert!(engine.tick("Q").unwrap().cycled);

        let log = engine.input.log.lock().unwrap();
        assert_eq!(log.as_slice(), &["cycle:Q"]);
    }
}
