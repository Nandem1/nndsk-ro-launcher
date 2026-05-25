use std::path::Path;

use serde::Deserialize;

use crate::autopot::config::AutopotConfig;
use crate::domain::{default_profile, ClientProfile};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileJsonEntry {
    id: String,
    label: String,
    exe_names: Vec<String>,
    hp_base: String,
    name_address: String,
}

pub fn parse_profiles_json(raw: &str) -> Result<Vec<ClientProfile>, String> {
    let entries: Vec<ProfileJsonEntry> =
        serde_json::from_str(raw).map_err(|e| format!("client_profiles.json: {e}"))?;

    entries
        .into_iter()
        .map(|entry| {
            Ok(ClientProfile {
                id: entry.id,
                label: entry.label,
                exe_names: entry.exe_names,
                hp_base: parse_hex(&entry.hp_base)?,
                name_address: parse_hex(&entry.name_address)?,
            })
        })
        .collect()
}

pub fn resolve_profile(
    profiles: &[ClientProfile],
    exe_path: &str,
    config: &AutopotConfig,
) -> ClientProfile {
    if let Some(override_hex) = &config.hp_base_override {
        if let Ok(hp_base) = parse_hex(override_hex) {
            let default = default_profile();
            return ClientProfile {
                hp_base,
                name_address: default.name_address,
                ..default
            };
        }
    }

    if let Some(profile_id) = &config.profile_id {
        if let Some(found) = profiles.iter().find(|p| p.id == *profile_id) {
            return found.clone();
        }
    }

    let exe_name = Path::new(exe_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(exe_path);

    if let Some(found) = profiles.iter().find(|p| p.matches_exe(exe_name)) {
        return found.clone();
    }

    default_profile()
}

pub fn parse_hex(value: &str) -> Result<u32, String> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u32::from_str_radix(hex, 16).map_err(|e| format!("invalid hex '{value}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_by_exe_name() {
        let profiles = vec![ClientProfile {
            id: "test".into(),
            label: "Test".into(),
            exe_names: vec!["HoneyRO.exe".into()],
            hp_base: 0x10DCE10,
            name_address: 0x10DF5D8,
        }];

        let config = AutopotConfig::default();
        let resolved = resolve_profile(&profiles, "/games/HoneyRO.exe", &config);
        assert_eq!(resolved.id, "test");
    }
}
