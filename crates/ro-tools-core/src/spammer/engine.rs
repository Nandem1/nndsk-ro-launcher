use crate::error::ToolsError;
use crate::ports::InputWriter;
use crate::spammer::config::SpammerConfig;
use crate::spammer::keys::is_valid_spammer_key;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpammerTick {
    pub cycled: bool,
}

pub struct SpammerEngine<I: InputWriter> {
    input: I,
    config: SpammerConfig,
}

impl<I: InputWriter> SpammerEngine<I> {
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

    /// Ciclo IPC-mode: KEYDOWN → click → KEYUP.
    /// El grab lo hace ro-inputd; cada ciclo es un press discreto independiente del estado físico.
    pub fn tick(&mut self, key: &str) -> Result<SpammerTick, ToolsError> {
        let key = key.trim();
        if !is_valid_spammer_key(key) {
            return Err(ToolsError::Input {
                key: key.to_string(),
                message: "tecla spammer no soportada".into(),
            });
        }

        self.input.key_down(key)?;
        self.input.click_left()?;
        self.input.key_up(key)?;

        Ok(SpammerTick { cycled: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::InputWriter;
    use std::sync::Mutex;

    struct MockInput {
        log: Mutex<Vec<String>>,
    }

    impl InputWriter for MockInput {
        fn key_down(&self, key: &str) -> Result<(), ToolsError> {
            self.log.lock().unwrap().push(format!("down:{key}"));
            Ok(())
        }

        fn key_up(&self, key: &str) -> Result<(), ToolsError> {
            self.log.lock().unwrap().push(format!("up:{key}"));
            Ok(())
        }

        fn press_key(&self, key: &str) -> Result<(), ToolsError> {
            self.log.lock().unwrap().push(format!("key:{key}"));
            Ok(())
        }

        fn click_left(&self) -> Result<(), ToolsError> {
            self.log.lock().unwrap().push("click".into());
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
            },
        );

        let tick = engine.tick("F2").unwrap();
        assert!(tick.cycled);

        let log = engine.input.log.lock().unwrap();
        assert_eq!(log.as_slice(), &["down:F2", "click", "up:F2"]);
    }

    #[test]
    fn spammer_rejects_invalid_key() {
        let input = MockInput {
            log: Mutex::new(vec![]),
        };
        let mut engine = SpammerEngine::new(input, SpammerConfig::default());
        assert!(engine.tick("Q").is_err());
    }
}
