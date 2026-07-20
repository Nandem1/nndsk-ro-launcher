use serde::{Deserialize, Serialize};

use crate::utils::default_system_wine;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub default_runner: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_runner: default_system_wine(),
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.default_runner.trim().is_empty() {
            return Err("El runner por defecto no puede estar vacío".to_string());
        }
        Ok(())
    }
}
