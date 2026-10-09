use ro_tools_core::SpammerConfig;
use tauri::AppHandle;

use crate::tools::input::InputGateway;

use super::SpammerHandle;

/// Arranca el loop de spam (solo input virtual; no requiere PID).
pub async fn start_session(
    app: AppHandle,
    handle: &SpammerHandle,
    input: InputGateway,
    config: SpammerConfig,
) -> Result<(), String> {
    if !input.is_prepared() {
        return Err("Spammer necesita uinput: revisa los permisos de /dev/uinput y /dev/input/event*. El juego no requiere esos permisos".into());
    }
    handle.start(app, input, config).await
}
