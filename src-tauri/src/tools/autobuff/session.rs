use super::AutobuffHandle;
use crate::models::server::ServerConfig;
use crate::state::GameProcessHandle;
use crate::tools::autopot::{load_profiles, resolve_profile};
use crate::tools::input::InputGateway;
use crate::tools::memory_sessions::{
    memory_start_error, MemorySessionRegistry, MEMORY_SESSION_MISSING,
};
use crate::utils::emit_tool_log_opt;
use tauri::AppHandle;

pub async fn start_session(
    app: AppHandle,
    handle: &AutobuffHandle,
    input: InputGateway,
    game: &GameProcessHandle,
    memory: &MemorySessionRegistry,
    server: ServerConfig,
) -> Result<(), String> {
    let profile = resolve_profile(&load_profiles(), &server.executable_path, &server.autopot);
    let identity = game.sole_running_identity()?;
    emit_tool_log_opt(
        Some(&app),
        format!("[AutoBuff] Cliente PID={}", identity.pid),
    );
    let session = memory
        .get(&identity)
        .ok_or_else(|| MEMORY_SESSION_MISSING.to_string())?;
    let access = session.access();
    if !access.usable() {
        return Err(memory_start_error(access));
    }
    let profile_memory = session.profile_memory(Some(profile.hp_base));
    if !input.is_prepared() {
        return Err("AutoBuff no puede iniciar: uinput no fue preparado antes de Wine".into());
    }
    handle
        .start(
            app,
            identity,
            session,
            server.autobuff,
            profile,
            input,
            access,
            profile_memory,
        )
        .await
}
