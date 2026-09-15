use tauri::AppHandle;

use crate::models::memory::MemoryAccess;
use crate::models::server::ServerConfig;
use crate::state::GameProcessHandle;
use crate::tools::autopot::{load_profiles, resolve_profile, AutopotHandle};
use crate::tools::input::InputGateway;
use crate::tools::memory_sessions::{
    memory_start_error, MemorySessionRegistry, MEMORY_SESSION_MISSING,
};
use crate::utils::{emit_tool_log_opt, resolve_server_prefix};

/// Orquesta arranque de AutoPot: resuelve identidad, valida memoria compartida y delega al servicio.
pub async fn start_session(
    app: AppHandle,
    handle: &AutopotHandle,
    input: InputGateway,
    game: &GameProcessHandle,
    memory: &MemorySessionRegistry,
    server: ServerConfig,
) -> Result<(), String> {
    let prefix = resolve_server_prefix(Some(&server))?.path;
    let profiles = load_profiles();
    let profile = resolve_profile(&profiles, &server.executable_path, &server.autopot);

    let identity = game.sole_running_identity()?;
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Cliente PID={} exe={} prefix={prefix}",
            identity.pid, server.executable_path
        ),
    );
    emit_tool_log_opt(
        Some(&app),
        format!(
            "[AutoPot] Perfil '{}' HP={:#x} name={:#x}",
            profile.label, profile.hp_base, profile.name_address
        ),
    );

    let session = memory
        .get(&identity)
        .ok_or_else(|| MEMORY_SESSION_MISSING.to_string())?;
    let access = session.access();
    if !access.usable() {
        return Err(memory_start_error(access));
    }
    let profile_memory = session.profile_memory(Some(profile.hp_base));
    if profile_memory == crate::models::memory::ProfileMemory::InvalidRead
        && access == MemoryAccess::ProcessVmReadv
    {
        emit_tool_log_opt(
            Some(&app),
            "[AutoPot] Perfil: lectura inválida en la dirección configurada",
        );
    }

    if !input.is_prepared() {
        return Err("AutoPot no puede iniciar: uinput no fue preparado antes de Wine".into());
    }

    handle
        .start(
            app,
            identity,
            session,
            server.autopot.clone(),
            profile,
            input,
            access,
            profile_memory,
        )
        .await
}
