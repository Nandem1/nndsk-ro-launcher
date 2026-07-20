use std::path::Path;

use crate::models::dependency::{DependencyStatus, RuntimeCheck, RuntimeCheckSeverity};
use crate::models::server::ServerConfig;
use crate::tools::runners::{managed_proton_path, managed_runtime_ready, MANAGED_RUNNER_LABEL};
use crate::tools::server_tools;
use crate::utils::audio;
use crate::utils::{
    ensure_custom_setup_allowed, ensure_managed_path_safe, ensure_managed_reset_allowed,
    inspect_prefix, is_dxvk_installed, manifest_matches_location, manifest_matches_runner,
    proton_runner_vkd3d_companions_available, proton_vkd3d_companions_available,
    resolve_server_prefix, resolve_server_wine_context_with_runner, resolve_wine_context,
    runtime_prefix_blockers, winetricks_available, PrefixScope, WineContext, PREFIX_SCHEMA_VERSION,
};
use ro_tools_linux::{detect_input_permissions, detect_uinput_permissions};

pub async fn check_dependencies(
    server: Option<ServerConfig>,
    legacy_runner: Option<String>,
) -> Result<DependencyStatus, String> {
    if !managed_runtime_ready() {
        return managed_runtime_pending(server.as_ref());
    }
    let ctx = resolve_context(server.as_ref(), legacy_runner).await?;
    let health = inspect_prefix(&ctx.prefix);
    let externally_managed = ctx.location.scope == PrefixScope::Custom;
    let prefix_configured = if externally_managed {
        health.structure_ok
    } else {
        health.configured
    };

    let runner_kind = ctx.resolved.kind_label().to_string();
    let runner_path = ctx.resolved.runner_path().to_string_lossy().to_string();
    let mut prefix_issues = health.issues.clone();

    let manifest_compatible = match &health.manifest {
        Some(manifest) if manifest.runner_kind == "unknown" => {
            prefix_issues.push(
                "El manifiesto es legacy y no identifica el runner; rearma el entorno antes de instalar componentes"
                    .to_string(),
            );
            false
        }
        Some(manifest) => {
            let matches = manifest.schema_version == PREFIX_SCHEMA_VERSION
                && manifest_matches_location(manifest, &ctx.location)
                && manifest_matches_runner(manifest, &runner_kind, &runner_path);
            if !matches {
                prefix_issues.push(format!(
                    "El entorno fue creado con otro runner ({})",
                    manifest.runner_path
                ));
            }
            matches
        }
        None => externally_managed,
    };

    let vkd3d_ok = ctx
        .resolved
        .proton_root()
        .map(|root| proton_vkd3d_companions_available(&ctx.prefix, root))
        .unwrap_or(true);
    let runner_vkd3d_ok = ctx
        .resolved
        .proton_root()
        .map(proton_runner_vkd3d_companions_available)
        .unwrap_or(true);
    if !vkd3d_ok {
        prefix_issues
            .push("Proton no expone las DLL compañeras libvkd3d para x86 y x86_64".to_string());
    }

    let dxvk = if let Some(root) = ctx.resolved.proton_root() {
        is_dxvk_installed(&ctx.prefix) || proton_dxvk_available(root)
    } else {
        is_dxvk_installed(&ctx.prefix)
    };
    let mut blockers = runtime_prefix_blockers(&ctx, &health);
    let webview2_required = server.as_ref().is_some_and(server_tools::requires_webview2);
    let missing_components = server
        .as_ref()
        .map(|server| server_tools::missing_runtime_components(server, Path::new(&ctx.prefix)))
        .unwrap_or_default();
    let webview2_missing = missing_components
        .iter()
        .any(|issue| issue.contains("WebView2"));
    blockers.extend(missing_components.iter().cloned());
    blockers.sort();
    blockers.dedup();
    prefix_issues.extend(blockers.iter().cloned());
    prefix_issues.sort();
    prefix_issues.dedup();

    let runner_setup_available = ctx.resolved.is_proton() || winetricks_available();
    let path_safe = ensure_managed_path_safe(&ctx.location).is_ok()
        && ensure_custom_setup_allowed(&ctx.location).is_ok();
    let incompatible_manifest = health.manifest.as_ref().is_some_and(|manifest| {
        manifest.schema_version != PREFIX_SCHEMA_VERSION
            || !manifest_matches_location(manifest, &ctx.location)
            || (manifest.runner_kind != "unknown"
                && !manifest_matches_runner(manifest, &runner_kind, &runner_path))
    });
    let mut required_verbs = vec!["vcrun2019", "d3dx9", "corefonts"];
    if !ctx.resolved.is_proton() {
        required_verbs.push("dxvk");
    }
    if webview2_required {
        required_verbs.push("webview2");
    }
    let missing_verbs: Vec<_> = required_verbs
        .iter()
        .copied()
        .filter(|verb| !ctx.resolved.supports_winetricks_verb(verb))
        .collect();
    let required_verbs_available = missing_verbs.is_empty();
    if !missing_verbs.is_empty() {
        prefix_issues.push(format!(
            "El winetricks efectivo no incluye: {}",
            missing_verbs.join(", ")
        ));
    }
    let runner_unknown = health
        .manifest
        .as_ref()
        .is_some_and(|manifest| manifest.runner_kind == "unknown");
    let prefix_root = Path::new(&ctx.prefix);
    let managed_unclaimed = ctx.location.managed
        && prefix_root.is_dir()
        && prefix_root
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some())
        && health.manifest.is_none()
        && !health.legacy_marker;
    if managed_unclaimed {
        prefix_issues.push(
            "El directorio administrado contiene datos sin un manifiesto válido; no se adoptará"
                .to_string(),
        );
    }
    let requires_rebuild =
        incompatible_manifest || (ctx.location.managed && (health.legacy_marker || runner_unknown));
    let rebuild_allowed = requires_rebuild
        && ctx.location.managed
        && ensure_managed_reset_allowed(&ctx.location).is_ok();
    let can_setup = runner_setup_available
        && required_verbs_available
        && path_safe
        && runner_vkd3d_ok
        && (!requires_rebuild || rebuild_allowed)
        && !managed_unclaimed;
    let prefix_ok = blockers.is_empty() && prefix_configured && manifest_compatible && vkd3d_ok;
    let ready_to_launch = prefix_ok;

    let prefix_warning = (!prefix_issues.is_empty()).then(|| prefix_issues.join(" · "));
    let dxvk_warning = if dxvk {
        None
    } else if prefix_configured {
        Some(
            "DXVK D3D9 no está disponible para las arquitecturas del entorno; WineD3D aún puede funcionar"
                .to_string(),
        )
    } else {
        Some("DXVK se comprobará después de construir el entorno".to_string())
    };

    let (audio_ok, audio_driver, audio_warning, audio_stack) =
        audio::dependency_audio_fields(&ctx.prefix, prefix_configured);
    let input_perms = detect_input_permissions();
    let uinput_perms = detect_uinput_permissions();

    let mut checks = vec![RuntimeCheck {
        id: "runner".to_string(),
        severity: if runner_vkd3d_ok {
            RuntimeCheckSeverity::Ok
        } else {
            RuntimeCheckSeverity::Error
        },
        message: format!("Runner {} mediante {}", runner_path, runner_kind),
        remediation: (!runner_vkd3d_ok)
            .then(|| "Cambia o reinstala la distribución Proton".to_string()),
    }];
    if !missing_verbs.is_empty() {
        checks.push(RuntimeCheck {
            id: "winetricks-verbs".to_string(),
            severity: RuntimeCheckSeverity::Error,
            message: format!(
                "Faltan componentes instalables en el winetricks efectivo: {}",
                missing_verbs.join(", ")
            ),
            remediation: Some(
                "Actualiza winetricks o selecciona una distribución Proton que incluya esos verbos"
                    .to_string(),
            ),
        });
    }
    checks.push(RuntimeCheck {
        id: "prefix".to_string(),
        severity: if prefix_ok {
            if prefix_warning.is_some() {
                RuntimeCheckSeverity::Warning
            } else {
                RuntimeCheckSeverity::Ok
            }
        } else if prefix_configured {
            RuntimeCheckSeverity::Error
        } else {
            RuntimeCheckSeverity::Pending
        },
        message: if prefix_ok {
            format!("Entorno listo en {}", ctx.prefix)
        } else {
            format!("Entorno pendiente o incompatible en {}", ctx.prefix)
        },
        remediation: prefix_warning.clone(),
    });
    checks.push(RuntimeCheck {
        id: "dxvk".to_string(),
        severity: if dxvk {
            RuntimeCheckSeverity::Ok
        } else {
            RuntimeCheckSeverity::Warning
        },
        message: if dxvk {
            "Direct3D 9 disponible mediante DXVK".to_string()
        } else {
            "DXVK D3D9 no detectado".to_string()
        },
        remediation: dxvk_warning.clone(),
    });
    if ctx.resolved.is_proton() {
        checks.push(RuntimeCheck {
            id: "vkd3d-runtime".to_string(),
            severity: if runner_vkd3d_ok {
                RuntimeCheckSeverity::Ok
            } else {
                RuntimeCheckSeverity::Error
            },
            message: if runner_vkd3d_ok {
                "DLL compañeras VKD3D disponibles".to_string()
            } else {
                "Faltan DLL compañeras VKD3D".to_string()
            },
            remediation: (!runner_vkd3d_ok).then(|| {
                "Reinstala o cambia la distribución Proton y rearma este entorno".to_string()
            }),
        });
    }
    if webview2_missing {
        checks.push(RuntimeCheck {
            id: "webview2".to_string(),
            severity: RuntimeCheckSeverity::Error,
            message: "Microsoft Edge WebView2 Runtime no está instalado".to_string(),
            remediation: if required_verbs_available {
                Some("Repara o rearma este entorno para instalar WebView2".to_string())
            } else {
                Some("Selecciona un Proton cuyo winetricks incluya el verbo webview2".to_string())
            },
        });
    }

    Ok(DependencyStatus {
        // Campos legacy: `wine` ahora significa que la estrategia seleccionada se resolvió.
        wine: true,
        winetricks: if ctx.resolved.is_proton() {
            true
        } else {
            winetricks_available()
        },
        dxvk,
        prefix_configured,
        audio_ok,
        audio_driver,
        audio_stack,
        audio_warning,
        input_group_ok: input_perms.ok,
        input_group_warning: input_perms.warning,
        uinput_input_ok: uinput_perms.ok,
        uinput_input_warning: uinput_perms.warning,
        prefix_ok,
        prefix_warning,
        dxvk_ok: dxvk || !prefix_configured,
        dxvk_warning,
        runner_kind: MANAGED_RUNNER_LABEL.to_string(),
        runner_ok: runner_vkd3d_ok,
        runner_warning: (!runner_vkd3d_ok)
            .then(|| "La distribución Proton no contiene el runtime VKD3D completo".to_string()),
        prefix_path: ctx.prefix,
        prefix_scope: ctx.location.scope.as_str().to_string(),
        prefix_managed: ctx.location.managed,
        ready_to_launch,
        can_setup,
        can_reset: runner_setup_available
            && required_verbs_available
            && runner_vkd3d_ok
            && ensure_managed_reset_allowed(&ctx.location).is_ok(),
        checks,
    })
}

fn managed_runtime_pending(server: Option<&ServerConfig>) -> Result<DependencyStatus, String> {
    let location = resolve_server_prefix(server)?;
    let health = inspect_prefix(&location.path);
    let prefix_root = Path::new(&location.path);
    let managed_unclaimed = location.managed
        && prefix_root.is_dir()
        && prefix_root
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some())
        && health.manifest.is_none()
        && !health.legacy_marker;
    let path_safe = ensure_managed_path_safe(&location).is_ok();
    let managed_runner_path = managed_proton_path();
    let manifest_compatible = health.manifest.as_ref().is_some_and(|manifest| {
        manifest.schema_version == PREFIX_SCHEMA_VERSION
            && manifest_matches_location(manifest, &location)
            && manifest_matches_runner(
                manifest,
                "proton",
                managed_runner_path.to_string_lossy().as_ref(),
            )
    });
    let requires_rebuild = health.legacy_marker
        || health
            .manifest
            .as_ref()
            .is_some_and(|_| !manifest_compatible);
    let reset_allowed = ensure_managed_reset_allowed(&location).is_ok();
    let can_reset = health.configured && reset_allowed;
    let can_setup = path_safe && !managed_unclaimed && (!requires_rebuild || reset_allowed);
    let runtime_warning =
        "El runtime Ragnarok se descargará y verificará al preparar el entorno".to_string();
    let prefix_warning = if managed_unclaimed {
        Some(
            "El directorio administrado contiene datos sin un manifiesto válido; no se modificará"
                .to_string(),
        )
    } else if requires_rebuild && !reset_allowed {
        Some(
            "El manifiesto del entorno no permite reconstruirlo de forma automática y segura"
                .to_string(),
        )
    } else if health.configured {
        Some("El entorno se validará con el runtime administrado antes de jugar".to_string())
    } else {
        Some("El entorno aislado se construirá automáticamente antes de jugar".to_string())
    };
    let (audio_ok, audio_driver, audio_warning, audio_stack) =
        audio::dependency_audio_fields(&location.path, health.configured);
    let input_perms = detect_input_permissions();
    let uinput_perms = detect_uinput_permissions();

    Ok(DependencyStatus {
        wine: false,
        winetricks: false,
        dxvk: false,
        prefix_configured: health.configured,
        audio_ok,
        audio_driver,
        audio_stack,
        audio_warning,
        input_group_ok: input_perms.ok,
        input_group_warning: input_perms.warning,
        uinput_input_ok: uinput_perms.ok,
        uinput_input_warning: uinput_perms.warning,
        prefix_ok: false,
        prefix_warning,
        dxvk_ok: false,
        dxvk_warning: Some("DXVK está incluido en el runtime administrado".to_string()),
        runner_kind: MANAGED_RUNNER_LABEL.to_string(),
        runner_ok: false,
        runner_warning: Some(runtime_warning.clone()),
        prefix_path: location.path.clone(),
        prefix_scope: location.scope.as_str().to_string(),
        prefix_managed: location.managed,
        ready_to_launch: false,
        can_setup,
        can_reset,
        checks: vec![
            RuntimeCheck {
                id: "runner".to_string(),
                severity: RuntimeCheckSeverity::Pending,
                message: format!("{MANAGED_RUNNER_LABEL} pendiente"),
                remediation: Some(runtime_warning),
            },
            RuntimeCheck {
                id: "prefix".to_string(),
                severity: RuntimeCheckSeverity::Pending,
                message: format!("Entorno pendiente en {}", location.path),
                remediation: None,
            },
            RuntimeCheck {
                id: "dxvk".to_string(),
                severity: RuntimeCheckSeverity::Pending,
                message: "DXVK se instalará con el runtime".to_string(),
                remediation: None,
            },
        ],
    })
}

async fn resolve_context(
    server: Option<&ServerConfig>,
    legacy_runner: Option<String>,
) -> Result<WineContext, String> {
    if server.is_some() {
        resolve_server_wine_context_with_runner(server, legacy_runner).await
    } else {
        resolve_wine_context(None, legacy_runner).await
    }
}

fn proton_dxvk_available(proton_root: &Path) -> bool {
    let dxvk = proton_root.join("files/lib/wine/dxvk");
    dxvk.join("x86_64-windows/d3d9.dll").is_file() && dxvk.join("i386-windows/d3d9.dll").is_file()
}
