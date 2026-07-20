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
    let exe_name = Path::new(exe_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(exe_path);

    let mut resolved = config
        .profile_id
        .as_ref()
        .and_then(|profile_id| profiles.iter().find(|profile| profile.id == *profile_id))
        .or_else(|| {
            profiles
                .iter()
                .find(|profile| profile.matches_exe(exe_name))
        })
        .cloned()
        .unwrap_or_else(default_profile);

    if let Some(hp_base) = config
        .hp_base_override
        .as_deref()
        .and_then(|address| parse_hex(address).ok())
    {
        resolved.hp_base = hp_base;
    }
    if let Some(name_address) = config
        .name_address_override
        .as_deref()
        .and_then(|address| parse_hex(address).ok())
    {
        resolved.name_address = name_address;
    }
    resolved
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

    #[test]
    fn applies_hp_and_name_overrides_to_the_selected_profile() {
        let profiles = vec![ClientProfile {
            id: "test".into(),
            label: "Test".into(),
            exe_names: vec!["Client.exe".into()],
            hp_base: 0x1000,
            name_address: 0x2000,
        }];
        let config = AutopotConfig {
            profile_id: Some("test".into()),
            hp_base_override: Some("0x3000".into()),
            name_address_override: Some("0x4000".into()),
            ..Default::default()
        };

        let resolved = resolve_profile(&profiles, "/games/Client.exe", &config);

        assert_eq!(resolved.hp_base, 0x3000);
        assert_eq!(resolved.name_address, 0x4000);
        assert_eq!(resolved.id, "test");
    }
}
