use std::collections::BTreeSet;

use crate::models::dependency::{DependencyStatus, RuntimeCheckSeverity};
use crate::models::environment::{
    EnvironmentAction, EnvironmentAssessment, EnvironmentDomain, EnvironmentIntent,
    EnvironmentIssue, HostFamily, LaunchReadiness, ToolProvider,
};

use super::guidance;

pub(crate) fn preparation_only(code: &str) -> bool {
    matches!(
        code,
        "host-cabextract"
            | "host-unzip"
            | "host-downloader"
            | "host-winetricks"
            | "winetricks-verbs"
    )
}

fn domain(code: &str) -> EnvironmentDomain {
    match code {
        "host-cabextract" | "host-unzip" | "host-downloader" | "host-winetricks"
        | "winetricks-verbs" => EnvironmentDomain::Tools,
        "runner" | "vkd3d-runtime" => EnvironmentDomain::Runner,
        "prefix" | "webview2" => EnvironmentDomain::Prefix,
        "dxvk" => EnvironmentDomain::Graphics,
        "input-group" | "uinput" | "audio" => EnvironmentDomain::Feature,
        id if id.starts_with("gepard") => EnvironmentDomain::Compatibility,
        _ => EnvironmentDomain::System,
    }
}

fn title(code: &str) -> &str {
    match code {
        "host-cabextract" => "Herramienta cabextract",
        "host-unzip" => "Herramienta unzip",
        "host-downloader" => "Herramienta de descarga",
        "host-winetricks" | "winetricks-verbs" => "Componentes de Winetricks",
        "host-elf32" => "Bibliotecas de 32 bits",
        "host-python" => "Python para Proton",
        "host-ca" => "Certificados de descarga",
        "host-vulkan" => "Vulkan del sistema",
        "host-x11" => "Sesión gráfica",
        "host-home" => "Directorio del usuario",
        "runner" | "vkd3d-runtime" => "Runtime seleccionado",
        "prefix" => "Entorno del servidor",
        "webview2" => "WebView2",
        "dxvk" => "Componentes gráficos",
        "gepard-runner" => "Compatibilidad de Gepard",
        _ => "Comprobación del entorno",
    }
}

pub(crate) fn assess(status: &DependencyStatus) -> EnvironmentAssessment {
    assess_for_family(status, guidance::host_family())
}

fn assess_for_family(status: &DependencyStatus, family: HostFamily) -> EnvironmentAssessment {
    let issues: Vec<_> = status
        .checks
        .iter()
        .filter(|check| check.severity != RuntimeCheckSeverity::Ok)
        .map(|check| {
            let domain = domain(&check.id);
            let affects = if preparation_only(&check.id) {
                vec![EnvironmentIntent::Prepare, EnvironmentIntent::Rebuild]
            } else if matches!(
                domain,
                EnvironmentDomain::Feature | EnvironmentDomain::Compatibility
            ) {
                // Compatibility recommendations and optional tools never become launch gates.
                vec![EnvironmentIntent::Feature]
            } else {
                vec![
                    EnvironmentIntent::Launch,
                    EnvironmentIntent::Prepare,
                    EnvironmentIntent::Rebuild,
                ]
            };
            EnvironmentIssue {
                code: check.id.clone(),
                domain,
                severity: check.severity,
                affects,
                title: title(&check.id).to_string(),
                message: check.message.clone(),
                remediation: check.remediation.clone(),
            }
        })
        .collect();
    let mut actions = vec![EnvironmentAction::Recheck];
    if !status.ready_to_launch && status.can_setup {
        actions.push(EnvironmentAction::Prepare);
    }
    if issues.iter().any(|issue| {
        issue.code.starts_with("host-") && issue.severity == RuntimeCheckSeverity::Error
    }) {
        let packages: BTreeSet<_> = issues
            .iter()
            .filter(|issue| issue.severity == RuntimeCheckSeverity::Error)
            .flat_map(|issue| guidance::packages_for(&issue.code, family))
            .map(|package| package.to_string())
            .collect();
        actions.push(EnvironmentAction::HostGuide {
            family,
            packages: packages.into_iter().collect(),
        });
    }
    EnvironmentAssessment {
        schema_version: 1,
        provider: if status.runner_kind == "proton" {
            ToolProvider::ProtonTools
        } else {
            ToolProvider::WineHostTools
        },
        launch: if status.ready_to_launch {
            LaunchReadiness::Ready
        } else if status.can_setup {
            LaunchReadiness::NeedsPreparation
        } else {
            LaunchReadiness::Blocked
        },
        preparation_available: status.can_setup,
        rebuild_available: status.can_reset,
        issues,
        actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dependency::RuntimeCheck;

    pub(crate) fn ready_wine() -> DependencyStatus {
        DependencyStatus {
            wine: true,
            winetricks: true,
            dxvk: true,
            prefix_configured: true,
            audio_ok: true,
            audio_driver: "pulse".into(),
            audio_stack: "pulse".into(),
            audio_warning: None,
            input_group_ok: true,
            input_group_warning: None,
            uinput_input_ok: true,
            uinput_input_warning: None,
            prefix_ok: true,
            prefix_warning: None,
            dxvk_ok: true,
            dxvk_warning: None,
            runner_kind: "wine".into(),
            runner_ok: true,
            runner_warning: None,
            prefix_path: "/unused/prefix".into(),
            prefix_scope: "isolated".into(),
            prefix_managed: true,
            ready_to_launch: true,
            can_setup: true,
            can_reset: true,
            checks: vec![],
            environment: None,
            runtime_plan: None,
            compatibility: None,
        }
    }

    pub(crate) fn missing(code: &str) -> RuntimeCheck {
        RuntimeCheck {
            id: code.into(),
            severity: RuntimeCheckSeverity::Error,
            message: format!("Falta {code}"),
            remediation: Some("Instala la herramienta".into()),
        }
    }

    #[test]
    fn missing_preparation_tools_preserve_launch_and_runner_identity() {
        let mut status = ready_wine();
        super::super::host::apply_host_checks(
            &mut status,
            vec![missing("host-cabextract"), missing("host-unzip")],
        );
        assert!(status.ready_to_launch);
        assert!(status.runner_ok);
        assert!(status.runner_warning.is_none());
        assert!(!status.can_setup && !status.can_reset);
        let assessment = assess_for_family(&status, HostFamily::Arch);
        assert_eq!(assessment.launch, LaunchReadiness::Ready);
        assert!(assessment
            .issues
            .iter()
            .all(|issue| !issue.affects.contains(&EnvironmentIntent::Launch)));
        let json = serde_json::to_value(assessment).unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["provider"], "wineHostTools");
        assert_eq!(json["actions"][1]["kind"], "hostGuide");
        assert_eq!(
            json["actions"][1]["packages"],
            serde_json::json!(["cabextract", "unzip"])
        );
        assert!(json.get("preparation_available").is_none());
    }

    #[test]
    fn fresh_prefix_reports_every_blocker_and_recovers_after_recheck() {
        let mut status = ready_wine();
        status.ready_to_launch = false;
        super::super::host::apply_host_checks(
            &mut status,
            vec![
                missing("host-cabextract"),
                missing("host-unzip"),
                missing("host-downloader"),
            ],
        );
        let assessment = assess_for_family(&status, HostFamily::Arch);
        assert_eq!(assessment.launch, LaunchReadiness::Blocked);
        assert_eq!(assessment.issues.len(), 3);
        assert!(!assessment
            .actions
            .iter()
            .any(|action| matches!(action, EnvironmentAction::Prepare)));
        // A new inspection is authoritative; old failures are not sticky state.
        let mut repaired = ready_wine();
        repaired.ready_to_launch = false;
        let assessment = assess_for_family(&repaired, HostFamily::Arch);
        assert_eq!(assessment.launch, LaunchReadiness::NeedsPreparation);
        assert!(assessment
            .actions
            .iter()
            .any(|action| matches!(action, EnvironmentAction::Prepare)));
    }

    #[test]
    fn missing_launch_capability_blocks_without_marking_runner_corrupt() {
        let mut status = ready_wine();
        super::super::host::apply_host_checks(&mut status, vec![missing("host-elf32")]);
        assert!(!status.ready_to_launch && !status.can_setup && !status.can_reset);
        assert!(status.runner_ok && status.prefix_ok);
        assert_eq!(
            assess_for_family(&status, HostFamily::Other).launch,
            LaunchReadiness::Blocked
        );
    }

    #[test]
    fn compatibility_warnings_do_not_change_launch_or_offer_runner_migration() {
        let mut status = ready_wine();
        status.checks.push(RuntimeCheck {
            id: "gepard-runner".into(),
            severity: RuntimeCheckSeverity::Warning,
            message: "Hash desconocido".into(),
            remediation: Some("Conserva un prefix separado al probar runners".into()),
        });
        let assessment = assess_for_family(&status, HostFamily::Other);
        assert_eq!(assessment.launch, LaunchReadiness::Ready);
        assert_eq!(
            assessment.issues[0].domain,
            EnvironmentDomain::Compatibility
        );
        assert_eq!(assessment.issues[0].title, "Compatibilidad de Gepard");
        assert_eq!(assessment.actions.len(), 1);
        assert!(matches!(assessment.actions[0], EnvironmentAction::Recheck));
    }
}
