//! Preflight of concrete host capabilities, not distribution/package detection.
use crate::models::dependency::{DependencyStatus, RuntimeCheck, RuntimeCheckSeverity};
use crate::utils::{executable_in_path, sanitize_appimage_env, winetricks_path};
use std::path::Path;
use std::time::Duration;

const PYTHON_PROBE: &str = r#"
import sys, json, ssl, lzma, bz2, zlib, socket, ctypes, fcntl
if sys.version_info < (3,10): raise RuntimeError('UMU requires Python >= 3.10')
ca = ssl.create_default_context().cert_store_stats()['x509_ca'] > 0
try:
 ctypes.CDLL('libvulkan.so.1')
 vulkan = True
except OSError:
 vulkan = False
print(json.dumps({'ca': ca, 'vulkan': vulkan}))
"#;

fn check(
    id: &str,
    severity: RuntimeCheckSeverity,
    message: &str,
    remediation: &str,
) -> RuntimeCheck {
    RuntimeCheck {
        id: id.into(),
        severity,
        message: message.into(),
        remediation: Some(remediation.into()),
    }
}

pub(crate) fn selection_uses_proton(server: Option<&str>, default: Option<&str>) -> bool {
    let selected = server
        .filter(|s| !s.trim().is_empty())
        .or_else(|| default.filter(|s| !s.trim().is_empty()));
    selected.is_none_or(|s| Path::new(s).file_name().is_some_and(|s| s == "proton"))
}

pub(crate) fn selection_uses_managed_wine716(server: Option<&str>, default: Option<&str>) -> bool {
    server
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| default.map(str::trim).filter(|s| !s.is_empty()))
        .is_some_and(|s| crate::tools::runners::is_managed_wine716_path(Path::new(s)))
}

pub(crate) fn wine716_loader_check(available: bool) -> Option<RuntimeCheck> {
    (!available).then(|| check("host-elf32", RuntimeCheckSeverity::Error,
        "El loader ELF i386 /lib/ld-linux.so.2 requerido por el artifact Wine 7.16 no está disponible",
        "Activa multilib y repara el loader de 32 bits del host (Arch: lib32-glibc). Un archivo Wine existente no garantiza que su intérprete ELF esté instalado"))
}

pub(crate) fn selected_wine716_loader_check(required: bool) -> Option<RuntimeCheck> {
    required
        .then(|| {
            wine716_loader_check(crate::utils::is_executable_file(Path::new(
                "/lib/ld-linux.so.2",
            )))
        })
        .flatten()
}

pub(crate) async fn host_checks(proton: bool, provisioning: bool) -> Vec<RuntimeCheck> {
    let mut checks = Vec::new();
    if std::env::var_os("HOME").is_none_or(|home| {
        let home = Path::new(&home);
        !home.is_absolute() || !home.is_dir()
    }) {
        checks.push(check("host-home", RuntimeCheckSeverity::Error,
            "HOME no identifica un directorio personal absoluto existente",
            "Inicia el launcher como tu usuario de escritorio, con HOME válido; no uses sudo ni crees prefixes en rutas del sistema"));
    }
    if std::env::var("DISPLAY")
        .ok()
        .is_none_or(|value| value.is_empty())
    {
        checks.push(check("host-x11", RuntimeCheckSeverity::Error,
            "No hay DISPLAY X11 disponible para Wine/Proton",
            "Inicia el launcher dentro de tu sesión gráfica. En Wayland comprueba XWayland; no cambies permisos con xhost"));
    }
    if proton {
        let result = probe_python(executable_in_path("python3")).await;
        match result {
            Err(error) => checks.push(check("host-python", RuntimeCheckSeverity::Error,
                &format!("UMU necesita Python 3.10+ del host con ssl/lzma/bz2/ctypes: {error}"),
                "Instala o repara Python del host (Arch: python). No hace falta instalar UMU ni módulos pip: el zipapp los incluye")),
            Ok((ca, vulkan)) => {
                if !ca { checks.push(check("host-ca", RuntimeCheckSeverity::Error,
                    "Python no dispone de un trust store CA para las descargas HTTPS de UMU",
                    "Repara el trust store del host y SSL_CERT_FILE/SSL_CERT_DIR. Arch: ca-certificates y ca-certificates-utils; no instales certificados dentro de Wine")); }
                if !vulkan { checks.push(check("host-vulkan", RuntimeCheckSeverity::Error,
                    "El loader Vulkan x86_64 libvulkan.so.1 no se puede cargar",
                    "Instala el loader y el driver Vulkan para tu GPU (Arch: vulkan-icd-loader + driver correspondiente). Esto no verifica todavía el ICD ni Vulkan 32-bit")); }
            }
        }
    } else if provisioning {
        // Provisioning tools aren't a launch dependency of an already prepared Wine prefix.
        // Don't infer old/new WoW64 from a user-selected binary's name.
        for tool in ["cabextract", "unzip"] {
            if executable_in_path(tool).is_none() {
                checks.push(check(&format!("host-{tool}"), RuntimeCheckSeverity::Error,
                    &format!("El provisioning de Wine necesita {tool} en PATH"),
                    &format!("Instala {tool} en el host. Proton incluye su propio proveedor; Wine portable no incluye el container")));
            }
        }
        if executable_in_path("curl").is_none() && executable_in_path("wget").is_none() {
            checks.push(check(
                "host-downloader",
                RuntimeCheckSeverity::Error,
                "Winetricks de Wine necesita curl o wget en PATH",
                "Instala curl o wget en el host",
            ));
        }
        if winetricks_path().is_none()
            && !crate::utils::is_executable_file(
                &crate::tools::runners::managed_proton_path()
                    .parent()
                    .unwrap()
                    .join("protonfixes/winetricks"),
            )
        {
            checks.push(check("host-winetricks", RuntimeCheckSeverity::Error,
                "Wine 7.16 no tiene un script winetricks disponible",
                "Instala winetricks en el host o prepara primero nndsk-ro-proton, que incluye el script. No se descargará Proton como dependencia oculta del fallback"));
        }
    }
    checks
}

async fn probe_python(python: Option<std::path::PathBuf>) -> Result<(bool, bool), String> {
    let python = python.ok_or_else(|| "python3 no encontrado en el PATH efectivo".to_string())?;
    let mut command = tokio::process::Command::new(python);
    sanitize_appimage_env(&mut command);
    // Use the same CA override as the actual Proton invocation, not an inherited conflicting one.
    let ca = "/etc/ssl/certs/ca-certificates.crt";
    if Path::new(ca).is_file() {
        command.env("SSL_CERT_FILE", ca);
    }
    command.args(["-c", PYTHON_PROBE]).kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .map_err(|_| "el probe de Python superó 5 segundos".to_string())?
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        // Do not copy arbitrary inherited paths/environment into this preflight result.
        return Err(format!("el probe stdlib/SSL terminó con {}", output.status));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "respuesta inválida del probe Python".to_string())?;
    match (value["ca"].as_bool(), value["vulkan"].as_bool()) {
        (Some(ca), Some(vulkan)) => Ok((ca, vulkan)),
        _ => Err("respuesta incompleta del probe Python".into()),
    }
}

pub(crate) fn host_blocker(checks: &[RuntimeCheck]) -> Option<String> {
    checks
        .iter()
        .find(|check| check.severity == RuntimeCheckSeverity::Error)
        .map(|check| match &check.remediation {
            Some(remediation) => format!("{}: {}. {remediation}", check.id, check.message),
            None => format!("{}: {}", check.id, check.message),
        })
}

pub(super) fn apply_host_checks(status: &mut DependencyStatus, mut checks: Vec<RuntimeCheck>) {
    if let Some(error) = host_blocker(&checks) {
        status.ready_to_launch = false;
        status.can_setup = false;
        status.runner_ok = false;
        status.runner_warning = Some(error);
    }
    checks.append(&mut status.checks);
    status.checks = checks;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn missing_python_is_a_host_failure_not_missing_registry_files() {
        assert!(probe_python(None).await.unwrap_err().contains("PATH"));
        let check = check(
            "host-python",
            RuntimeCheckSeverity::Error,
            "Python missing",
            "install python",
        );
        assert_eq!(
            host_blocker(&[check]),
            Some("host-python: Python missing. install python".into())
        );
    }
    #[test]
    fn wine_override_does_not_require_umu_or_python() {
        assert!(!selection_uses_proton(
            Some("/fallback/bin/wine"),
            Some("/primary/proton")
        ));
        assert!(selection_uses_proton(Some("  "), Some("/primary/proton")));
        assert!(selection_uses_proton(None, None));
    }

    #[test]
    fn missing_legacy_elf_interpreter_is_a_host_error_not_a_corrupt_artifact() {
        assert!(wine716_loader_check(true).is_none());
        let check = wine716_loader_check(false).unwrap();
        assert_eq!(check.id, "host-elf32");
        assert_eq!(check.severity, RuntimeCheckSeverity::Error);
        assert!(host_blocker(&[check]).unwrap().contains("lib32-glibc"));
        assert!(selected_wine716_loader_check(false).is_none());
    }

    #[tokio::test]
    async fn python_capability_probe_distinguishes_missing_ca_loader_and_broken_interpreter() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "ro-host-probe-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&dir).unwrap();
        let python = dir.join("python3");
        for (json, expected) in [
            ("{\"ca\":false,\"vulkan\":true}", (false, true)),
            ("{\"ca\":true,\"vulkan\":false}", (true, false)),
        ] {
            std::fs::write(&python, format!("#!/bin/sh\nprintf '%s\\n' '{json}'\n")).unwrap();
            std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(probe_python(Some(python.clone())).await.unwrap(), expected);
        }
        std::fs::write(&python, "#!/bin/sh\nexit 3\n").unwrap();
        assert!(probe_python(Some(python.clone()))
            .await
            .unwrap_err()
            .contains("exit status: 3"));
        std::fs::write(&python, "#!/bin/sh\nprintf '{}\\n'\n").unwrap();
        assert!(probe_python(Some(python))
            .await
            .unwrap_err()
            .contains("incompleta"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
