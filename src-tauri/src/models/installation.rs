use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationBinding {
    pub installation_id: String,
    pub prefix_path: String,
    pub prefix_fingerprint: String,
}

impl InstallationBinding {
    pub fn validate(&self) -> Result<(), String> {
        uuid::Uuid::parse_str(&self.installation_id)
            .map_err(|_| "El identificador de instalación no es válido")?;
        absolute_path(&self.prefix_path)?;
        if self.prefix_fingerprint.len() != 64
            || !self
                .prefix_fingerprint
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err("La identidad del entorno de instalación no es válida".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallationStatus {
    Pending,
    Preparing,
    Running,
    Stopping,
    AwaitingClient,
    Failed,
    Interrupted,
    Completed,
}

impl InstallationStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Preparing | Self::Running | Self::Stopping)
    }

    pub fn can_restart(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Failed | Self::Interrupted | Self::AwaitingClient
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationDraft {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub installer_path: String,
    #[serde(default)]
    pub destination_path: Option<String>,
    pub runner: String,
    pub require_webview2: bool,
    pub status: InstallationStatus,
    pub binding: Option<InstallationBinding>,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
}

impl InstallationDraft {
    pub fn validate(&self) -> Result<(), String> {
        uuid::Uuid::parse_str(&self.id).map_err(|_| "La instalación no tiene un ID válido")?;
        if self.server_id != format!("installed-{}", self.id) {
            return Err("La instalación no conserva el identificador de servidor".into());
        }
        validate_name(&self.name)?;
        validate_exe(&self.installer_path)?;
        if let Some(destination) = &self.destination_path {
            absolute_path(destination)?;
        }
        absolute_path(&self.runner)?;
        if let Some(binding) = &self.binding {
            binding.validate()?;
            if binding.installation_id != self.id {
                return Err("El entorno pertenece a otra instalación".into());
            }
        }
        if matches!(
            self.status,
            InstallationStatus::Running
                | InstallationStatus::AwaitingClient
                | InstallationStatus::Completed
        ) && self.binding.is_none()
        {
            return Err("La instalación no tiene un entorno verificado".into());
        }
        if matches!(
            self.status,
            InstallationStatus::AwaitingClient | InstallationStatus::Completed
        ) && self.exit_code != Some(0)
        {
            return Err("El instalador no terminó correctamente".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInstallation {
    pub name: String,
    pub installer_path: String,
    #[serde(default)]
    pub destination_path: Option<String>,
    pub runner: String,
    #[serde(default)]
    pub require_webview2: bool,
}

pub fn absolute_path(value: &str) -> Result<(), String> {
    if !Path::new(value).is_absolute()
        || value == "/"
        || value.contains('\0')
        || value.len() > 4096
        || value.trim() != value
        || Path::new(value)
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("Selecciona una ruta absoluta válida".into());
    }
    Ok(())
}

pub fn validate_exe(value: &str) -> Result<(), String> {
    absolute_path(value)?;
    if !Path::new(value)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err("Selecciona un archivo .exe".into());
    }
    Ok(())
}

pub fn validate_name(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 80 || value.contains('\0') {
        return Err("El nombre debe tener entre 1 y 80 caracteres".into());
    }
    Ok(())
}
