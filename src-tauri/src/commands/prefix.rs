use std::path::Path;

use tauri::{AppHandle, State};

use crate::models::server::ServerConfig;
use crate::state::GameState;
use crate::tools::prefix;
use crate::tools::runners::ensure_managed_runtime;
use crate::tools::server_tools;
use crate::utils::{
    ensure_custom_setup_allowed, ensure_managed_path_safe, ensure_managed_reset_allowed,
    inspect_prefix, manifest_matches_location, manifest_matches_runner,
    proton_runner_vkd3d_companions_available, resolve_server_wine_context_with_runner,
    resolve_wine_context, WineContext, PREFIX_SCHEMA_VERSION,
};

#[tauri::command]
pub async fn setup_prefix(
    app: AppHandle,
    state: State<'_, GameState>,
    server: Option<ServerConfig>,
    runner: Option<String>,
) -> Result<(), String> {
    ensure_managed_runtime(&app).await?;
    let ctx = resolve_context(server.as_ref(), runner).await?;
    let requirements = runtime_requirements(server.as_ref(), &ctx);
    validate_requirement_support(&ctx, requirements)?;
    ensure_managed_path_safe(&ctx.location)?;
    ensure_custom_setup_allowed(&ctx.location)?;
    let health = inspect_prefix(&ctx.prefix);
    let root = Path::new(&ctx.prefix);
    let nonempty = root.is_dir()
        && root
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some());
    if ctx.location.managed && nonempty && health.manifest.is_none() && !health.legacy_marker {
        return Err(
            "El directorio administrado contiene datos pero no tiene un manifiesto válido; no se adoptará ni modificará"
                .to_string(),
        );
    }
    let runner_unknown = health
        .manifest
        .as_ref()
        .is_some_and(|manifest| manifest.runner_kind == "unknown");
    let mut rebuild_managed = ctx.location.managed && (health.legacy_marker || runner_unknown);
    if let Some(manifest) = &health.manifest {
        if manifest.schema_version != PREFIX_SCHEMA_VERSION
            || !manifest_matches_location(manifest, &ctx.location)
        {
            if ctx.location.managed {
                rebuild_managed = true;
            } else {
                return Err(
                    "El manifiesto del entorno no coincide con esta ruta o servidor; no se modificará"
                        .to_string(),
                );
            }
        }
        let runner_path = ctx.resolved.runner_path().to_string_lossy();
        if manifest.runner_kind != "unknown"
            && !manifest_matches_runner(manifest, ctx.resolved.kind_label(), runner_path.as_ref())
        {
            if ctx.location.managed {
                rebuild_managed = true;
            } else {
                return Err(
                    "El entorno personalizado pertenece a otro runner y no se modificará"
                        .to_string(),
                );
            }
        }
        let required_graphics = requirements.dxvk.manifest_component();
        if ctx.location.managed
            && !manifest
                .components
                .iter()
                .any(|component| component == required_graphics)
        {
            rebuild_managed = true;
        }
    }
    if rebuild_managed {
        ensure_managed_reset_allowed(&ctx.location)?;
        return prefix::reset_runtime_prefix(
            &app,
            &state.game,
            &state.sessions,
            &ctx,
            requirements,
        )
        .await;
    }
    prefix::setup_runtime_prefix(&app, &state.game, &state.sessions, &ctx, requirements).await
}

#[tauri::command]
pub async fn reset_prefix(
    app: AppHandle,
    state: State<'_, GameState>,
    server: Option<ServerConfig>,
    runner: Option<String>,
) -> Result<(), String> {
    ensure_managed_runtime(&app).await?;
    let ctx = resolve_context(server.as_ref(), runner).await?;
    let requirements = runtime_requirements(server.as_ref(), &ctx);
    validate_requirement_support(&ctx, requirements)?;
    ensure_managed_reset_allowed(&ctx.location)?;
    prefix::reset_runtime_prefix(&app, &state.game, &state.sessions, &ctx, requirements).await
}

fn runtime_requirements(
    server: Option<&ServerConfig>,
    ctx: &WineContext,
) -> prefix::RuntimeRequirements {
    let webview2 = server.is_some_and(server_tools::requires_webview2);
    prefix::RuntimeRequirements {
        webview2,
        dxvk: prefix::DxvkProvision::for_runner(&ctx.resolved),
    }
}

fn validate_requirement_support(
    ctx: &WineContext,
    requirements: prefix::RuntimeRequirements,
) -> Result<(), String> {
    if let Some(root) = ctx.resolved.proton_root() {
        if !proton_runner_vkd3d_companions_available(root) {
            return Err(
                "La distribución Proton no incluye las DLL compañeras VKD3D para ambas arquitecturas"
                    .to_string(),
            );
        }
    }

    let mut verbs = vec!["vcrun2019", "d3dx9", "corefonts"];
    if requirements.dxvk == prefix::DxvkProvision::Winetricks {
        verbs.push("dxvk");
    }
    if requirements.webview2 {
        verbs.push("webview2");
    }
    let missing: Vec<_> = verbs
        .into_iter()
        .filter(|verb| !ctx.resolved.supports_winetricks_verb(verb))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "El winetricks efectivo de este runner no incluye: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

async fn resolve_context(
    server: Option<&ServerConfig>,
    legacy_runner: Option<String>,
) -> Result<WineContext, String> {
    if let Some(server) = server {
        server.validate()?;
        resolve_server_wine_context_with_runner(Some(server), legacy_runner).await
    } else {
        resolve_wine_context(None, legacy_runner).await
    }
}
