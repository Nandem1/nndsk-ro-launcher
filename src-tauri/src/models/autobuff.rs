use ro_tools_core::AutobuffConfig;
use serde::{Deserialize, Serialize};

use super::memory::{MemoryAccess, ProfileMemory};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutobuffStatusEvent {
    pub active: bool,
    pub active_statuses: usize,
    pub last_applied_rule: Option<String>,
    pub delay_ms: u64,
    pub error: Option<String>,
    pub memory_access: Option<MemoryAccess>,
    pub profile_memory: Option<ProfileMemory>,
}

impl Default for AutobuffStatusEvent {
    fn default() -> Self {
        Self {
            active: false,
            active_statuses: 0,
            last_applied_rule: None,
            delay_ms: AutobuffConfig::default().delay_ms,
            error: None,
            memory_access: None,
            profile_memory: None,
        }
    }
}
