use ro_tools_core::{AutobuffConfig, AutopotConfig, SpammerConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub executable_path: String,
    pub patcher_path: Option<String>,
    pub wine_prefix: Option<String>,
    pub runner: Option<String>,
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
        }
        if let Some(runner) = &self.runner {
            validate_non_empty("El runner", runner)?;
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
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> ServerConfig {
        ServerConfig {
            id: "server-1".into(),
            name: "Test RO".into(),
            executable_path: "/games/test/Ragexe.exe".into(),
            patcher_path: None,
            wine_prefix: None,
            runner: None,
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
}
