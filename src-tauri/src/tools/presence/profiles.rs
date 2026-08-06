use std::path::Path;

use ro_tools_core::{parse_presence_profiles_json, PresenceMemoryProfile};
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
}
