use ro_tools_core::AutopotConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopotStatusEvent {
    pub active: bool,
    pub effective_delay_ms: u64,
    pub cur_hp: u32,
    pub max_hp: u32,
    pub cur_sp: u32,
    pub max_sp: u32,
    pub hp_percent: u32,
    pub sp_percent: u32,
    pub character_name: String,
    pub error: Option<String>,
}

impl Default for AutopotStatusEvent {
    fn default() -> Self {
        Self {
            active: false,
            effective_delay_ms: AutopotConfig::default().delay_ms,
            cur_hp: 0,
            max_hp: 0,
            cur_sp: 0,
            max_sp: 0,
            hp_percent: AutopotConfig::default().hp_percent,
            sp_percent: AutopotConfig::default().sp_percent,
            character_name: String::new(),
            error: None,
        }
    }
}
