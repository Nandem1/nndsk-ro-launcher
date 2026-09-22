pub mod config;
pub mod engine;
pub mod keys;

pub use config::{GearSwitchConfig, GearSwitchRule, ShiftModeConfig, SpammerConfig};
pub use engine::{SpammerEngine, SpammerTick};
