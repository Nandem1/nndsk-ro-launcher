use ro_tools_core::{parse_profiles_json, ClientProfile};
use std::sync::OnceLock;

static PROFILES: OnceLock<Vec<ClientProfile>> = OnceLock::new();

pub fn load_profiles() -> Vec<ClientProfile> {
    PROFILES
        .get_or_init(|| {
            let raw = include_str!("../../../resources/client_profiles.json");
            parse_profiles_json(raw).unwrap_or_else(|_| vec![ro_tools_core::domain::default_profile()])
        })
        .clone()
}

pub use ro_tools_core::resolve_profile;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_profiles() {
        let profiles = load_profiles();
        assert!(!profiles.is_empty());
        assert_eq!(profiles[0].hp_base, 0x10DCE10);
        assert_eq!(profiles[0].name_address, 0x10DF5D8);
    }
}
