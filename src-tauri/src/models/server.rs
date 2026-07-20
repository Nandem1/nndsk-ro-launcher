use ro_tools_core::{AutobuffConfig, AutopotConfig, SpammerConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::launch::{LaunchConfig, LaunchStrategy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrefixMode {
    Shared,
    Isolated,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub executable_path: String,
    pub patcher_path: Option<String>,
    pub wine_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_mode: Option<PrefixMode>,
    pub runner: Option<String>,
    #[serde(default, skip_serializing_if = "LaunchConfig::is_default")]
    pub launch: LaunchConfig,
    #[serde(default)]
    pub autopot: AutopotConfig,
    #[serde(default)]
    pub spammer: SpammerConfig,
    #[serde(default)]
    pub autobuff: AutobuffConfig,
}

impl ServerConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_required("El identificador del servidor", &self.id, 128)?;
        validate_required("El nombre del servidor", &self.name, 80)?;
        validate_exe_path("El ejecutable del cliente", &self.executable_path)?;

        if let Some(patcher_path) = &self.patcher_path {
            validate_exe_path("El patcher", patcher_path)?;
        }
        if let Some(prefix) = &self.wine_prefix {
            validate_non_empty("El WINEPREFIX", prefix)?;
            validate_custom_prefix_path(prefix)?;
        }
        match self.prefix_mode {
            Some(PrefixMode::Custom) if self.wine_prefix.is_none() => {
                return Err("El modo de prefijo custom requiere un WINEPREFIX".into());
            }
            Some(PrefixMode::Shared | PrefixMode::Isolated) if self.wine_prefix.is_some() => {
                return Err(
                    "winePrefix sólo puede definirse cuando prefixMode es custom o legacy".into(),
                );
            }
            _ => {}
        }
        if let Some(runner) = &self.runner {
            validate_non_empty("El runner", runner)?;
            if self.prefix_mode == Some(PrefixMode::Shared) {
                return Err("Un runner por servidor requiere un prefijo aislado o custom".into());
            }
        }
        self.launch.validate()?;
        if self.launch.strategy == LaunchStrategy::Patcher && self.patcher_path.is_none() {
            return Err("El inicio mediante patcher requiere configurar patcherPath".into());
        }
        self.autopot.validate().map_err(|error| error.to_string())?;
        self.spammer
            .validate_for_start()
            .map_err(|error| error.to_string())?;
        self.autobuff.validate().map_err(|error| error.to_string())
    }

    pub fn validate_executable_available(&self) -> Result<(), String> {
        self.validate()?;
        if !Path::new(&self.executable_path).is_file() {
            return Err(format!(
                "El ejecutable del cliente no existe: {}",
                self.executable_path
            ));
        }
        if self.launch.strategy == LaunchStrategy::Patcher {
            let patcher_path = self.patcher_path.as_deref().ok_or_else(|| {
                "El inicio mediante patcher requiere configurar patcherPath".to_string()
            })?;
            if !Path::new(patcher_path).is_file() {
                return Err(format!("El patcher no existe: {patcher_path}"));
            }
        }
        Ok(())
    }

    pub fn effective_prefix_mode(&self) -> PrefixMode {
        // Los campos legacy siguen siendo deserializables para no romper la configuración,
        // pero cada servidor de Ragnarok se ejecuta siempre en su entorno administrado.
        PrefixMode::Isolated
    }
}

pub fn validate_servers(servers: &[ServerConfig]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for server in servers {
        server.validate()?;
        if !ids.insert(&server.id) {
            return Err(format!("El identificador '{}' está duplicado", server.id));
        }
    }
    Ok(())
}

fn validate_required(label: &str, value: &str, max_len: usize) -> Result<(), String> {
    validate_non_empty(label, value)?;
    if value.chars().count() > max_len {
        return Err(format!("{label} no puede superar {max_len} caracteres"));
    }
    Ok(())
}

fn validate_non_empty(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} no puede estar vacío"));
    }
    Ok(())
}

fn validate_exe_path(label: &str, path: &str) -> Result<(), String> {
    validate_non_empty(label, path)?;
    if !path.to_ascii_lowercase().ends_with(".exe") {
        return Err(format!("{label} debe apuntar a un archivo .exe"));
    }
    Ok(())
}

fn validate_custom_prefix_path(value: &str) -> Result<(), String> {
    let path = Path::new(value.trim());
    if !path.is_absolute() {
        return Err("El WINEPREFIX personalizado debe usar una ruta absoluta".to_string());
    }
    if path.parent().is_none() {
        return Err("El WINEPREFIX personalizado no puede ser la raíz del sistema".to_string());
    }
    if let Some(home) = std::env::var_os("HOME") {
        if path == Path::new(&home) {
            return Err(
                "El WINEPREFIX personalizado no puede ser el directorio personal".to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ContractFixtures {
        valid_server: ServerConfig,
        valid_launch_servers: Vec<ServerConfig>,
        invalid_servers: Vec<InvalidServerFixture>,
    }

    #[derive(Deserialize)]
    struct InvalidServerFixture {
        server: ServerConfig,
    }

    fn server() -> ServerConfig {
        ServerConfig {
            id: "server-1".into(),
            name: "Test RO".into(),
            executable_path: "/games/test/Ragexe.exe".into(),
            patcher_path: None,
            wine_prefix: None,
            prefix_mode: None,
            runner: None,
            launch: Default::default(),
            autopot: Default::default(),
            spammer: Default::default(),
            autobuff: Default::default(),
        }
    }

    #[test]
    fn rejects_non_executable_client_path() {
        let mut invalid = server();
        invalid.executable_path = "/games/test/client".into();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_server_ids() {
        assert!(validate_servers(&[server(), server()]).is_err());
    }

    #[test]
    fn legacy_prefix_fields_always_resolve_to_an_isolated_environment() {
        let mut legacy = server();
        assert_eq!(legacy.effective_prefix_mode(), PrefixMode::Isolated);
        legacy.wine_prefix = Some("/prefixes/test".into());
        assert_eq!(legacy.effective_prefix_mode(), PrefixMode::Isolated);
        assert!(legacy.validate().is_ok());

        legacy.wine_prefix = None;
        legacy.runner = Some("/opt/wine/bin/wine".into());
        assert_eq!(legacy.effective_prefix_mode(), PrefixMode::Isolated);
        assert!(legacy.validate().is_ok());
    }

    #[test]
    fn custom_prefix_requires_a_safe_absolute_path() {
        let mut invalid = server();
        invalid.wine_prefix = Some("~/.wine".into());
        assert!(invalid.validate().is_err());
        invalid.wine_prefix = Some("/".into());
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn default_launch_and_prefix_mode_do_not_change_legacy_serialization() {
        let value = serde_json::to_value(server()).unwrap();
        assert!(value.get("launch").is_none());
        assert!(value.get("prefixMode").is_none());
    }

    #[test]
    fn matches_shared_server_contract_fixtures() {
        let fixtures: ContractFixtures = serde_json::from_str(include_str!(
            "../../../contract-fixtures/server-configs.json"
        ))
        .unwrap();
        assert!(fixtures.valid_server.validate().is_ok());
        for server in fixtures.valid_launch_servers {
            assert!(server.validate().is_ok());
        }
        for fixture in fixtures.invalid_servers {
            assert!(fixture.server.validate().is_err());
        }
    }

    #[test]
    fn legacy_server_ignores_removed_input_backend() {
        let server: ServerConfig = serde_json::from_value(serde_json::json!({
            "id": "legacy",
            "name": "Legacy RO",
            "executablePath": "/games/legacy/Ragexe.exe",
            "patcherPath": null,
            "winePrefix": null,
            "runner": null,
            "combatInputBackend": "legacy"
        }))
        .unwrap();
        assert!(serde_json::to_value(server)
            .unwrap()
            .get("combatInputBackend")
            .is_none());
    }
}
