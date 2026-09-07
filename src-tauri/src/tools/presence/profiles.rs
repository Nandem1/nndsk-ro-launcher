use std::path::Path;

use ro_tools_core::{
    parse_hex, parse_presence_profiles_json, resolve_presence_memory_profiles, AutopotConfig,
    PresenceAddressOverrides, PresenceMemoryProfile,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::OnceLock;

static PROFILES: OnceLock<Vec<PresenceMemoryProfile>> = OnceLock::new();

pub fn load_profiles() -> &'static [PresenceMemoryProfile] {
    PROFILES
        .get_or_init(|| {
            let raw = include_str!("../../../resources/presence_profiles.json");
            parse_presence_profiles_json(raw).unwrap_or_else(|error| {
                eprintln!("[ro-launcher] presence_profiles.json parse error: {error}");
                Vec::new()
            })
        })
        .as_slice()
}

pub fn resolve_profile(exe_path: &str) -> Option<PresenceMemoryProfile> {
    let path = Path::new(exe_path);
    let exe_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(exe_path);
    let metadata = fs::metadata(path).ok()?;
    let image_size = metadata.len();
    let executable_sha256 = hash_file(path).ok()?;

    load_profiles()
        .iter()
        .find(|profile| {
            profile.image_size == image_size
                && profile
                    .executable_sha256
                    .eq_ignore_ascii_case(&executable_sha256)
                && profile.matches_exe(exe_name)
        })
        .cloned()
}

pub fn parse_address_override(value: Option<&str>) -> Option<u32> {
    value.and_then(|raw| parse_hex(raw).ok())
}

pub fn overrides_from_autopot(autopot: &AutopotConfig) -> PresenceAddressOverrides {
    PresenceAddressOverrides {
        name: parse_address_override(autopot.name_address_override.as_deref()),
        hp: parse_address_override(autopot.hp_base_override.as_deref()),
        level: parse_address_override(autopot.level_address_override.as_deref()),
        job: parse_address_override(autopot.job_level_address_override.as_deref()),
        map: parse_address_override(autopot.map_address_override.as_deref()),
    }
}

pub fn resolve_runtime_profiles(
    exe_path: &str,
    overrides: PresenceAddressOverrides,
) -> (Option<PresenceMemoryProfile>, Vec<PresenceMemoryProfile>) {
    resolve_presence_memory_profiles(resolve_profile(exe_path), overrides)
}

fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("no se pudo leer el ejecutable: {error}"))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_profiles_are_build_specific() {
        assert_eq!(load_profiles().len(), 2);
        assert!(load_profiles()
            .iter()
            .all(|profile| profile.module_base == 0x0040_0000));
    }

    #[test]
    fn missing_exe_falls_back_to_derived_overrides() {
        let (locked, candidates) = resolve_runtime_profiles(
            "/tmp/missing-ro-client.exe",
            PresenceAddressOverrides {
                name: Some(0x010D_F5D8),
                ..PresenceAddressOverrides::default()
            },
        );
        assert!(locked.is_none());
        assert!(candidates
            .iter()
            .any(|profile| profile.level_address == 0x010D_9400));
    }

    #[test]
    fn parse_address_override_accepts_hex_and_rejects_junk() {
        assert_eq!(parse_address_override(Some("0x10DF5D8")), Some(0x010D_F5D8));
        assert_eq!(parse_address_override(Some("not-hex")), None);
        assert_eq!(parse_address_override(None), None);
    }

    #[test]
    fn missing_exe_without_overrides_stays_empty() {
        let (locked, candidates) = resolve_runtime_profiles(
            "/tmp/missing-ro-client.exe",
            PresenceAddressOverrides::default(),
        );
        assert!(locked.is_none());
        assert!(candidates.is_empty());
    }

    #[test]
    fn klaipeda_style_overrides_without_level_map_stay_empty() {
        let (locked, candidates) = resolve_runtime_profiles(
            "/tmp/missing-ro-client.exe",
            PresenceAddressOverrides {
                name: Some(0x0177_AE00),
                hp: Some(0x0177_8190),
                ..PresenceAddressOverrides::default()
            },
        );
        assert!(locked.is_none());
        assert!(candidates.is_empty());
    }
}
