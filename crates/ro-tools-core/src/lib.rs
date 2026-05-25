//! Core domain and autopot logic — no OS dependencies.

pub mod autopot;
pub mod dgvoodoo;
pub mod domain;
pub mod error;
pub mod ports;
pub mod profiles;

pub use autopot::config::AutopotConfig;
pub use autopot::engine::{AutopotEngine, AutopotTick};
pub use domain::ClientProfile;
pub use error::ToolsError;
pub use ports::{InputWriter, MemoryReader};
pub use profiles::{parse_hex, parse_profiles_json, resolve_profile};
