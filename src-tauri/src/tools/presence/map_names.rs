use std::collections::HashMap;
use std::sync::OnceLock;

static MAP_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

fn load_map_names() -> &'static HashMap<String, String> {
    MAP_NAMES.get_or_init(|| {
        let raw = include_str!("../../../resources/map_names.json");
        match serde_json::from_str::<HashMap<String, String>>(raw) {
            Ok(map) => map,
            Err(error) => {
                eprintln!("[ro-launcher] map_names.json parse error: {error}");
                HashMap::new()
            }
        }
    })
}

/// Resolve a map ID (`malaya`, `gef_fild00`) to a Discord-friendly label.
/// Unknown IDs keep their original value for diagnostics.
pub fn display_map_name(map_id: &str) -> String {
    let key = map_id.trim();
    if key.is_empty() {
        return map_id.to_string();
    }

    let catalog = load_map_names();
    if let Some(label) = catalog.get(key) {
        return label.clone();
    }

    let lowered = key.to_ascii_lowercase();
    if lowered != key {
        if let Some(label) = catalog.get(&lowered) {
            return label.clone();
        }
    }

    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_confirmed_map_ids_to_friendly_names() {
        assert_eq!(display_map_name("malaya"), "Port Malaya");
        assert_eq!(display_map_name("gef_fild00"), "Geffen Field");
        assert_eq!(display_map_name("xmas"), "Lutie");
        assert_eq!(display_map_name("prontera"), "Prontera");
        assert_eq!(display_map_name("izlu2dun"), "Byalan Island");
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(display_map_name("Malaya"), "Port Malaya");
        assert_eq!(display_map_name("PRONTERA"), "Prontera");
    }

    #[test]
    fn preserves_unknown_map_ids_for_diagnostics() {
        assert_eq!(display_map_name("custom_map_01"), "custom_map_01");
    }

    #[test]
    fn catalog_covers_common_towns_and_fields() {
        let catalog = load_map_names();
        assert!(catalog.len() > 1000);
        assert_eq!(catalog.get("geffen").map(String::as_str), Some("Geffen"));
        assert_eq!(
            catalog.get("moc_fild01").map(String::as_str),
            Some("Sograt Desert")
        );
        assert_eq!(
            catalog.get("abyss_01").map(String::as_str),
            Some("Abyss Lake Underground Cave F1")
        );
    }
}
