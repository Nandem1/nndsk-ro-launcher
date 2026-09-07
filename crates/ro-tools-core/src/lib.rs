//! Core domain and autopot logic — no OS dependencies.

pub mod autobuff;
pub mod autopot;
pub mod dgvoodoo;
pub mod domain;
pub mod error;
pub mod ports;
pub mod presence;
pub mod profiles;
pub mod spammer;

pub use autobuff::config::{AutobuffConfig, AutobuffRule};
pub use autobuff::engine::{AutobuffEngine, AutobuffTick};
pub use autopot::config::AutopotConfig;
pub use autopot::engine::{AutopotEngine, AutopotTick};
pub use domain::ClientProfile;
pub use error::ToolsError;
pub use ports::{HeldKeyWriter, KeyPressWriter, MemoryReader, SpamCycleWriter};
pub use presence::{
    derive_presence_profile, derive_presence_profiles, explicit_presence_profile,
    map_label_matches, map_scan_needles, normalize_map_name, parse_presence_profiles_json,
    read_character_snapshot, resolve_presence_memory_profiles, CharacterSnapshot, CharacterState,
    PresenceAddressOverrides, PresenceLayoutFamily, PresenceMemoryProfile, RATHENA_2018_FAMILY,
    SAKURA_2025_FAMILY,
};
pub use profiles::{parse_hex, parse_profiles_json, resolve_profile};
pub use spammer::config::{GearSwitchConfig, GearSwitchRule, SpammerConfig};
pub use spammer::engine::{SpammerEngine, SpammerTick};
