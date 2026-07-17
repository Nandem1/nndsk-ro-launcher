use crate::autopot::config::AutopotConfig;
use crate::domain::ClientProfile;
use crate::error::ToolsError;
use crate::ports::{KeyPressWriter, MemoryReader};

/// Snapshot after one autopot cycle (DT_AP logic).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutopotTick {
    pub cur_hp: u32,
    pub max_hp: u32,
    pub cur_sp: u32,
    pub max_sp: u32,
    pub character_name: String,
    pub potted_hp: bool,
    pub potted_sp: bool,
    /// HP input sent by proactive mode, not by the configured HP threshold.
    pub proactive_hp_pulse: bool,
}

pub struct AutopotEngine<M: MemoryReader, I: KeyPressWriter> {
    memory: M,
    input: I,
    config: AutopotConfig,
    profile: ClientProfile,
    hp_pot_count: u32,
    tick_count: u64,
    cached_name: String,
}

impl<M: MemoryReader, I: KeyPressWriter> AutopotEngine<M, I> {
    pub fn new(memory: M, input: I, config: AutopotConfig, profile: ClientProfile) -> Self {
        Self {
            memory,
            input,
            config: config.clamped(),
            profile,
            hp_pot_count: 0,
            tick_count: 0,
            cached_name: String::new(),
        }
    }

    pub fn update_config(&mut self, config: AutopotConfig) {
        self.config = config.clamped();
        self.hp_pot_count = 0;
    }

    pub fn config(&self) -> &AutopotConfig {
        &self.config
    }

    pub fn profile(&self) -> &ClientProfile {
        &self.profile
    }

    /// One iteration of the DT_AP autopot loop.
    pub fn tick(&mut self) -> Result<AutopotTick, ToolsError> {
        self.tick_count += 1;

        let values = self.memory.read_u32_slice(self.profile.hp_base, 4)?;
        let [cur_hp, max_hp, cur_sp, max_sp] = values.as_slice() else {
            return Err(ToolsError::Other(format!(
                "AutoPot: lectura HP/SP incompleta ({} de 4)",
                values.len()
            )));
        };
        let (cur_hp, max_hp, cur_sp, max_sp) = (*cur_hp, *max_hp, *cur_sp, *max_sp);

        if self.tick_count == 1 || self.tick_count.is_multiple_of(20) {
            self.cached_name = self
                .memory
                .read_string(self.profile.name_address, 40)
                .unwrap_or_default();
        }
        let character_name = self.cached_name.clone();

        let mut tick = AutopotTick {
            cur_hp,
            max_hp,
            cur_sp,
            max_sp,
            character_name,
            ..Default::default()
        };

        if self.is_hp_below(cur_hp, max_hp) {
            self.pot_hp()?;
            tick.potted_hp = true;
            self.hp_pot_count += 1;

            if self.hp_pot_count == 3 {
                self.hp_pot_count = 0;
                if self.is_sp_below(cur_sp, max_sp) {
                    self.pot_sp()?;
                    tick.potted_sp = true;
                }
            }
        }

        if self.is_sp_below(cur_sp, max_sp) && !tick.potted_sp {
            self.pot_sp()?;
            tick.potted_sp = true;
        }

        // Keep regular HP/SP recovery ahead of proactive input. This avoids a
        // continuous HP pulse competing with an urgent potion action.
        if self.config.proactive_mode && !tick.potted_hp && !tick.potted_sp {
            self.pot_hp()?;
            tick.proactive_hp_pulse = true;
        }

        Ok(tick)
    }

    fn is_hp_below(&self, cur: u32, max: u32) -> bool {
        max > 0 && cur * 100 < self.config.hp_percent * max
    }

    fn is_sp_below(&self, cur: u32, max: u32) -> bool {
        max > 0 && cur * 100 < self.config.sp_percent * max
    }

    fn pot_hp(&self) -> Result<(), ToolsError> {
        self.input.press_key(&self.config.hp_key)
    }

    fn pot_sp(&self) -> Result<(), ToolsError> {
        self.input.press_key(&self.config.sp_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{KeyPressWriter, MemoryReader};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockMemory {
        data: HashMap<u32, u32>,
        name: String,
        slice_reads: Mutex<u64>,
    }

    impl MemoryReader for MockMemory {
        fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
            self.data
                .get(&address)
                .copied()
                .ok_or_else(|| ToolsError::Other(format!("missing {address:#x}")))
        }

        fn read_string(&self, _address: u32, _max_len: usize) -> Result<String, ToolsError> {
            Ok(self.name.clone())
        }

        fn read_u32_slice(&self, address: u32, len: usize) -> Result<Vec<u32>, ToolsError> {
            *self.slice_reads.lock().unwrap() += 1;
            (0..len)
                .map(|index| self.read_u32(address + index as u32 * 4))
                .collect()
        }
    }

    struct MockInput {
        pressed: Mutex<Vec<String>>,
    }

    impl KeyPressWriter for MockInput {
        fn press_key(&self, key: &str) -> Result<(), ToolsError> {
            self.pressed.lock().unwrap().push(key.to_string());
            Ok(())
        }
    }

    fn engine(hp: u32, max_hp: u32, sp: u32, max_sp: u32) -> AutopotEngine<MockMemory, MockInput> {
        let profile = ClientProfile {
            id: "test".into(),
            label: "test".into(),
            exe_names: vec![],
            hp_base: 0x1000,
            name_address: 0x2000,
        };
        let mut data = HashMap::new();
        data.insert(0x1000, hp);
        data.insert(0x1004, max_hp);
        data.insert(0x1008, sp);
        data.insert(0x100C, max_sp);

        AutopotEngine::new(
            MockMemory {
                data,
                name: "TestChar".into(),
                slice_reads: Mutex::new(0),
            },
            MockInput {
                pressed: Mutex::new(vec![]),
            },
            AutopotConfig {
                hp_percent: 80,
                sp_percent: 50,
                hp_key: "F8".into(),
                sp_key: "F9".into(),
                ..Default::default()
            },
            profile,
        )
    }

    #[test]
    fn pots_hp_when_below_threshold() {
        let mut e = engine(700, 1000, 500, 500);
        let tick = e.tick().unwrap();
        assert!(tick.potted_hp);
        assert!(!tick.potted_sp);
    }

    #[test]
    fn pots_sp_when_below_threshold() {
        let mut e = engine(900, 1000, 200, 500);
        let tick = e.tick().unwrap();
        assert!(tick.potted_sp);
    }

    #[test]
    fn proactive_mode_pulses_hp_when_no_recovery_is_needed() {
        let mut e = engine(1000, 1000, 500, 500);
        e.update_config(AutopotConfig {
            proactive_mode: true,
            ..e.config().clone()
        });

        let tick = e.tick().unwrap();

        assert!(tick.proactive_hp_pulse);
        assert!(!tick.potted_hp);
        assert!(!tick.potted_sp);
        assert_eq!(e.input.pressed.lock().unwrap().as_slice(), ["F8"]);
    }

    #[test]
    fn proactive_mode_does_not_compete_with_sp_recovery() {
        let mut e = engine(1000, 1000, 200, 500);
        e.update_config(AutopotConfig {
            proactive_mode: true,
            ..e.config().clone()
        });

        let tick = e.tick().unwrap();

        assert!(tick.potted_sp);
        assert!(!tick.proactive_hp_pulse);
        assert_eq!(e.input.pressed.lock().unwrap().as_slice(), ["F9"]);
    }

    #[test]
    fn reads_hp_and_sp_in_one_slice_operation() {
        let mut e = engine(900, 1000, 500, 500);
        e.tick().unwrap();
        assert_eq!(*e.memory.slice_reads.lock().unwrap(), 1);
    }

    #[test]
    fn third_hp_cycle_sends_sp_only_once() {
        let mut e = engine(700, 1000, 200, 500);
        e.tick().unwrap();
        e.tick().unwrap();
        e.tick().unwrap();
        let pressed = e.input.pressed.lock().unwrap();
        assert_eq!(pressed.iter().filter(|key| key.as_str() == "F9").count(), 3);
    }
}
