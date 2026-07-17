use std::time::Duration;

use ro_tools_core::ToolsError;

use crate::tools::input::GatewayWriter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearMode {
    Atk,
    Def,
}

impl GearMode {
    pub fn as_str(self) -> &'static str {
        match self {
            GearMode::Atk => "atk",
            GearMode::Def => "def",
        }
    }
}

/// Equipa un set presionando cada tecla (down+up, sin click) con `switch_delay_ms` entre ellas.
/// Pensado para ejecutarse en `spawn_blocking` (usa sleeps síncronos).
pub fn equip(
    writer: &GatewayWriter,
    keys: &[String],
    switch_delay_ms: u64,
) -> Result<(), ToolsError> {
    for (index, key) in keys.iter().enumerate() {
        if index > 0 {
            std::thread::sleep(Duration::from_millis(switch_delay_ms));
        }
        writer.press_key(key)?;
    }
    Ok(())
}
