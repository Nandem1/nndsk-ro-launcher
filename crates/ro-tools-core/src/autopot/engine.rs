use crate::autopot::config::AutopotConfig;
use crate::domain::ClientProfile;
use crate::error::ToolsError;
use crate::ports::{InputWriter, MemoryReader};

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
}

pub struct AutopotEngine<M: MemoryReader, I: InputWriter> {
    memory: M,
    input: I,
    config: AutopotConfig,
    profile: ClientProfile,
    hp_pot_count: u32,
    tick_count: u64,
    cached_name: String,
}

impl<M: MemoryReader, I: InputWriter> AutopotEngine<M, I> {
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

        let cur_hp = self.memory.read_u32(self.profile.hp_base)?;
        let max_hp = self.memory.read_u32(self.profile.max_hp_address())?;
        let cur_sp = self.memory.read_u32(self.profile.cur_sp_address())?;
        let max_sp = self.memory.read_u32(self.profile.max_sp_address())?;

        if self.tick_count == 1 || self.tick_count % 20 == 0 {
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

        if self.is_sp_below(cur_sp, max_sp) {
            self.pot_sp()?;
            tick.potted_sp = true;
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
    use crate::ports::{InputWriter, MemoryReader};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockMemory {
        data: HashMap<u32, u32>,
        name: String,
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
    }

    struct MockInput {
        pressed: Mutex<Vec<String>>,
    }

    impl InputWriter for MockInput {
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
}
