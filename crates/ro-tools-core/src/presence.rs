use std::time::Instant;

use serde::Deserialize;

use crate::error::ToolsError;
use crate::ports::MemoryReader;
use crate::profiles::parse_hex;

const MAX_TEXT_LEN: usize = 40;
const MAX_LEVEL: u32 = 300;
const MODULE_BASE: u32 = 0x0040_0000;
const RATHENA_NAME_FROM_HP: u32 = 0x27C8;
const INFINITY_NAME_MINUS_HP: u32 = 0x2A4C;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceMemoryProfile {
    pub id: String,
    pub exe_names: Vec<String>,
    pub executable_sha256: String,
    pub pe_build_timestamp: String,
    pub image_size: u64,
    pub module_base: u32,
    pub name_address: u32,
    pub level_address: u32,
    pub job_level_address: u32,
    pub map_address: u32,
}

/// Confirmed presence layout expressed as signed deltas from `name_address`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenceLayoutFamily {
    pub id: &'static str,
    pub name_minus_hp: Option<u32>,
    pub name_minus_level: u32,
    pub name_minus_job: u32,
    pub name_minus_map: u32,
}

pub const RATHENA_2018_FAMILY: PresenceLayoutFamily = PresenceLayoutFamily {
    id: "derived-rathena-2018",
    name_minus_hp: Some(RATHENA_NAME_FROM_HP),
    name_minus_level: 0x61D8,
    name_minus_job: 0x61D0,
    name_minus_map: 0x706C,
};

pub const SAKURA_2025_FAMILY: PresenceLayoutFamily = PresenceLayoutFamily {
    id: "derived-sakura-2025",
    name_minus_hp: None,
    name_minus_level: 0x6B78,
    name_minus_job: 0x6B70,
    name_minus_map: 0x6BBC,
};

const LAYOUT_FAMILIES: &[PresenceLayoutFamily] = &[RATHENA_2018_FAMILY, SAKURA_2025_FAMILY];

impl PresenceLayoutFamily {
    pub fn apply(self, name_address: u32) -> Option<PresenceMemoryProfile> {
        Some(PresenceMemoryProfile {
            id: self.id.to_string(),
            exe_names: Vec::new(),
            executable_sha256: String::new(),
            pe_build_timestamp: String::new(),
            image_size: 0,
            module_base: MODULE_BASE,
            name_address,
            level_address: name_address.checked_sub(self.name_minus_level)?,
            job_level_address: name_address.checked_sub(self.name_minus_job)?,
            map_address: name_address.checked_sub(self.name_minus_map)?,
        })
    }
}

impl PresenceMemoryProfile {
    pub fn matches_exe(&self, exe_name: &str) -> bool {
        let exe_lower = exe_name.to_ascii_lowercase();
        self.exe_names.iter().any(|pattern| {
            let pattern = pattern.to_ascii_lowercase();
            if pattern.contains('*') {
                glob_match(&pattern, &exe_lower)
            } else {
                pattern == exe_lower
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterState {
    Unknown,
    InGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSnapshot {
    pub character_name: Option<String>,
    pub level: Option<u32>,
    pub job_level: Option<u32>,
    pub map_name: Option<String>,
    pub state: CharacterState,
    pub sampled_at: Instant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresenceProfileJson {
    id: String,
    #[serde(default)]
    exe_names: Vec<String>,
    executable_sha256: String,
    pe_build_timestamp: String,
    image_size: u64,
    module_base: String,
    name_address: String,
    level_address: String,
    job_level_address: String,
    map_address: String,
}

pub fn parse_presence_profiles_json(raw: &str) -> Result<Vec<PresenceMemoryProfile>, String> {
    let entries: Vec<PresenceProfileJson> =
        serde_json::from_str(raw).map_err(|error| format!("presence_profiles.json: {error}"))?;

    entries
        .into_iter()
        .map(|entry| {
            Ok(PresenceMemoryProfile {
                id: entry.id,
                exe_names: entry.exe_names,
                executable_sha256: entry.executable_sha256,
                pe_build_timestamp: entry.pe_build_timestamp,
                image_size: entry.image_size,
                module_base: parse_hex(&entry.module_base)?,
                name_address: parse_hex(&entry.name_address)?,
                level_address: parse_hex(&entry.level_address)?,
                job_level_address: parse_hex(&entry.job_level_address)?,
                map_address: parse_hex(&entry.map_address)?,
            })
        })
        .collect()
}

pub fn derive_presence_profile(
    name_address: Option<u32>,
    hp_base: Option<u32>,
) -> Option<PresenceMemoryProfile> {
    let mut profiles = derive_presence_profiles(name_address, hp_base);
    (profiles.len() == 1).then(|| profiles.remove(0))
}

pub fn derive_presence_profiles(
    name_address: Option<u32>,
    hp_base: Option<u32>,
) -> Vec<PresenceMemoryProfile> {
    let Some(name_address) = name_address else {
        return Vec::new();
    };

    if let Some(hp) = hp_base {
        let Some(delta) = name_address.checked_sub(hp) else {
            return Vec::new();
        };
        if delta == INFINITY_NAME_MINUS_HP {
            return Vec::new();
        }
        return LAYOUT_FAMILIES
            .iter()
            .copied()
            .find(|family| family.name_minus_hp == Some(delta))
            .and_then(|family| family.apply(name_address))
            .into_iter()
            .collect();
    }

    LAYOUT_FAMILIES
        .iter()
        .copied()
        .filter_map(|family| family.apply(name_address))
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PresenceAddressOverrides {
    pub name: Option<u32>,
    pub hp: Option<u32>,
    pub level: Option<u32>,
    pub job: Option<u32>,
    pub map: Option<u32>,
}

pub fn explicit_presence_profile(
    overrides: PresenceAddressOverrides,
) -> Option<PresenceMemoryProfile> {
    let name_address = overrides.name?;
    let level_address = overrides.level?;
    let map_address = overrides.map?;
    Some(PresenceMemoryProfile {
        id: "scanned".into(),
        exe_names: Vec::new(),
        executable_sha256: String::new(),
        pe_build_timestamp: String::new(),
        image_size: 0,
        module_base: MODULE_BASE,
        name_address,
        level_address,
        job_level_address: overrides
            .job
            .unwrap_or_else(|| level_address.saturating_add(8)),
        map_address,
    })
}

pub fn resolve_presence_memory_profiles(
    hash_profile: Option<PresenceMemoryProfile>,
    overrides: PresenceAddressOverrides,
) -> (Option<PresenceMemoryProfile>, Vec<PresenceMemoryProfile>) {
    if let Some(profile) = hash_profile {
        return (Some(profile), Vec::new());
    }
    if let Some(profile) = explicit_presence_profile(overrides) {
        return (None, vec![profile]);
    }
    (None, derive_presence_profiles(overrides.name, overrides.hp))
}

pub fn map_scan_needles(raw: &str) -> Option<Vec<Vec<u8>>> {
    let name = normalize_map_name(raw)?;
    Some(vec![
        format!("{name}\0").into_bytes(),
        format!("{name}.rsw\0").into_bytes(),
    ])
}

pub fn map_label_matches(raw: &str, expected: &str) -> bool {
    normalize_map_name(raw).as_deref() == Some(expected)
}

pub fn read_character_snapshot<R: MemoryReader>(
    reader: &R,
    profile: &PresenceMemoryProfile,
) -> Result<CharacterSnapshot, ToolsError> {
    let mut successful_reads = 0;
    let mut last_error = None;

    let character_name = match reader.read_string(profile.name_address, MAX_TEXT_LEN) {
        Ok(value) => {
            successful_reads += 1;
            sanitize_text(&value)
        }
        Err(error) => {
            last_error = Some(error);
            None
        }
    };
    let level = match reader.read_u32(profile.level_address) {
        Ok(value) => {
            successful_reads += 1;
            valid_level(value)
        }
        Err(error) => {
            last_error = Some(error);
            None
        }
    };
    let job_level = match reader.read_u32(profile.job_level_address) {
        Ok(value) => {
            successful_reads += 1;
            valid_level(value)
        }
        Err(error) => {
            last_error = Some(error);
            None
        }
    };
    let map_name = match reader.read_string(profile.map_address, MAX_TEXT_LEN) {
        Ok(value) => {
            successful_reads += 1;
            normalize_map_name(&value)
        }
        Err(error) => {
            last_error = Some(error);
            None
        }
    };

    if successful_reads == 0 {
        return Err(last_error.unwrap_or_else(|| {
            ToolsError::Other("no se pudo leer ningún campo del personaje".into())
        }));
    }

    let state = if character_name.is_some() && level.is_some() && map_name.is_some() {
        CharacterState::InGame
    } else {
        CharacterState::Unknown
    };

    Ok(CharacterSnapshot {
        character_name,
        level,
        job_level,
        map_name,
        state,
        sampled_at: Instant::now(),
    })
}

fn valid_level(value: u32) -> Option<u32> {
    (1..=MAX_LEVEL).contains(&value).then_some(value)
}

fn sanitize_text(value: &str) -> Option<String> {
    let value: String = value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_TEXT_LEN)
        .collect();
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub fn normalize_map_name(value: &str) -> Option<String> {
    let value = sanitize_text(value)?.to_ascii_lowercase();
    let value = value
        .strip_suffix(".rsw")
        .or_else(|| value.strip_suffix(".gat"))
        .unwrap_or(&value);
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_@-".contains(character))
    {
        return None;
    }
    Some(value.to_string())
}

fn glob_match(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = text;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        let is_first = index == 0;
        let is_last = index == parts.len() - 1;
        if is_first && !pattern.starts_with('*') {
            if !rest.starts_with(part) {
                return false;
            }
            rest = &rest[part.len()..];
            continue;
        }
        if is_last && !pattern.ends_with('*') {
            return rest.ends_with(part);
        }

        let Some(position) = rest.find(part) else {
            return false;
        };
        rest = &rest[position + part.len()..];
    }

    true
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    struct FakeMemory {
        u32_values: HashMap<u32, u32>,
        strings: HashMap<u32, String>,
    }

    impl MemoryReader for FakeMemory {
        fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
            self.u32_values
                .get(&address)
                .copied()
                .ok_or_else(|| ToolsError::MemoryRead {
                    address,
                    message: "missing".into(),
                })
        }

        fn read_string(&self, address: u32, _max_len: usize) -> Result<String, ToolsError> {
            self.strings
                .get(&address)
                .cloned()
                .ok_or_else(|| ToolsError::MemoryRead {
                    address,
                    message: "missing".into(),
                })
        }
    }

    fn profile() -> PresenceMemoryProfile {
        PresenceMemoryProfile {
            id: "test".into(),
            exe_names: vec!["HoneyRO.exe".into()],
            executable_sha256: "hash".into(),
            pe_build_timestamp: "timestamp".into(),
            image_size: 1,
            module_base: 0x0040_0000,
            name_address: 0x1000,
            level_address: 0x1004,
            job_level_address: 0x1008,
            map_address: 0x100c,
        }
    }

    #[test]
    fn parses_hex_presence_profiles() {
        let raw = r#"[
          {
            "id": "test",
            "exeNames": ["HoneyRO.exe"],
            "executableSha256": "hash",
            "peBuildTimestamp": "timestamp",
            "imageSize": 42,
            "moduleBase": "0x00400000",
            "nameAddress": "0x1000",
            "levelAddress": "0x1004",
            "jobLevelAddress": "0x1008",
            "mapAddress": "0x100c"
          }
        ]"#;

        let profiles = parse_presence_profiles_json(raw).unwrap();
        assert_eq!(profiles[0].module_base, 0x0040_0000);
        assert_eq!(profiles[0].map_address, 0x100c);
    }

    #[test]
    fn reads_and_normalizes_a_complete_snapshot() {
        let profile = profile();
        let memory = FakeMemory {
            u32_values: HashMap::from([(0x1004, 99), (0x1008, 70)]),
            strings: HashMap::from([
                (0x1000, "  Character\u{0000} ".into()),
                (0x100c, "MALAYA.RSW".into()),
            ]),
        };

        let snapshot = read_character_snapshot(&memory, &profile).unwrap();
        assert_eq!(snapshot.character_name.as_deref(), Some("Character"));
        assert_eq!(snapshot.level, Some(99));
        assert_eq!(snapshot.job_level, Some(70));
        assert_eq!(snapshot.map_name.as_deref(), Some("malaya"));
        assert_eq!(snapshot.state, CharacterState::InGame);
    }

    #[test]
    fn wrong_layout_family_does_not_publish_false_ingame_data() {
        let honey_name = 0x010D_F5D8;
        let honey = RATHENA_2018_FAMILY.apply(honey_name).expect("honey");
        let sakura_on_honey = SAKURA_2025_FAMILY.apply(honey_name).expect("wrong family");
        let memory = FakeMemory {
            u32_values: HashMap::from([(honey.level_address, 10), (honey.job_level_address, 7)]),
            strings: HashMap::from([
                (honey.name_address, "Hero".into()),
                (honey.map_address, "prontera".into()),
            ]),
        };

        let valid = read_character_snapshot(&memory, &honey).unwrap();
        assert_eq!(valid.state, CharacterState::InGame);

        let invalid = read_character_snapshot(&memory, &sakura_on_honey).unwrap();
        assert_eq!(invalid.state, CharacterState::Unknown);
        assert!(invalid.level.is_none());
        assert!(invalid.map_name.is_none());
    }

    #[test]
    fn invalid_values_produce_an_unknown_snapshot_without_false_data() {
        let profile = profile();
        let memory = FakeMemory {
            u32_values: HashMap::from([(0x1004, 0), (0x1008, 500)]),
            strings: HashMap::from([(0x1000, "\u{0001}".into()), (0x100c, "payon/map".into())]),
        };

        let snapshot = read_character_snapshot(&memory, &profile).unwrap();
        assert_eq!(snapshot.character_name, None);
        assert_eq!(snapshot.level, None);
        assert_eq!(snapshot.job_level, None);
        assert_eq!(snapshot.map_name, None);
        assert_eq!(snapshot.state, CharacterState::Unknown);
    }

    #[test]
    fn matches_executable_globs_case_insensitively() {
        assert!(profile().matches_exe("honeyro.EXE"));
        assert!(!profile().matches_exe("other.exe"));
    }

    #[test]
    fn rathena_family_reproduces_honey_vas_from_name() {
        let profile = RATHENA_2018_FAMILY.apply(0x010D_F5D8).expect("honey name");
        assert_eq!(profile.name_address, 0x010D_F5D8);
        assert_eq!(profile.level_address, 0x010D_9400);
        assert_eq!(profile.job_level_address, 0x010D_9408);
        assert_eq!(profile.map_address, 0x010D_856C);
        assert_eq!(profile.module_base, 0x0040_0000);
    }

    #[test]
    fn sakura_family_reproduces_sakura_vas_from_name() {
        let profile = SAKURA_2025_FAMILY.apply(0x0160_2568).expect("sakura name");
        assert_eq!(profile.name_address, 0x0160_2568);
        assert_eq!(profile.level_address, 0x015F_B9F0);
        assert_eq!(profile.job_level_address, 0x015F_B9F8);
        assert_eq!(profile.map_address, 0x015F_B9AC);
    }

    #[test]
    fn name_only_tries_both_confirmed_families() {
        let honey = derive_presence_profiles(Some(0x010D_F5D8), None);
        assert!(honey.iter().any(|profile| {
            profile.id == RATHENA_2018_FAMILY.id
                && profile.level_address == 0x010D_9400
                && profile.map_address == 0x010D_856C
        }));
        assert!(honey
            .iter()
            .any(|profile| profile.id == SAKURA_2025_FAMILY.id));

        let sakura = derive_presence_profiles(Some(0x0160_2568), None);
        assert!(sakura.iter().any(|profile| {
            profile.id == SAKURA_2025_FAMILY.id
                && profile.level_address == 0x015F_B9F0
                && profile.map_address == 0x015F_B9AC
        }));
    }

    #[test]
    fn matching_rathena_hp_fingerprint_selects_only_that_family() {
        let profiles = derive_presence_profiles(Some(0x010D_F5D8), Some(0x010D_CE10));
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, RATHENA_2018_FAMILY.id);
        assert_eq!(profiles[0].level_address, 0x010D_9400);
        assert_eq!(
            derive_presence_profile(Some(0x010D_F5D8), Some(0x010D_CE10))
                .map(|profile| profile.map_address),
            Some(0x010D_856C)
        );
    }

    #[test]
    fn hp_only_does_not_derive_a_presence_profile() {
        assert!(derive_presence_profiles(None, Some(0x010D_CE10)).is_empty());
        assert!(derive_presence_profile(None, Some(0x010D_CE10)).is_none());
        assert!(derive_presence_profiles(None, Some(0x0146_F28C)).is_empty());
    }

    #[test]
    fn infinity_hp_and_name_do_not_derive_a_publishable_profile() {
        assert!(derive_presence_profiles(Some(0x0147_1CD8), Some(0x0146_F28C)).is_empty());
        assert!(derive_presence_profile(Some(0x0147_1CD8), Some(0x0146_F28C)).is_none());
    }

    #[test]
    fn hash_profile_wins_over_autopot_overrides() {
        let hashed = profile();
        let (locked, candidates) = resolve_presence_memory_profiles(
            Some(hashed.clone()),
            PresenceAddressOverrides {
                name: Some(0x010D_F5D8),
                hp: Some(0x010D_CE10),
                ..PresenceAddressOverrides::default()
            },
        );
        assert_eq!(
            locked.as_ref().map(|profile| profile.id.as_str()),
            Some("test")
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn overrides_derive_when_hash_profile_is_missing() {
        let (locked, candidates) = resolve_presence_memory_profiles(
            None,
            PresenceAddressOverrides {
                name: Some(0x010D_F5D8),
                hp: Some(0x010D_CE10),
                ..PresenceAddressOverrides::default()
            },
        );
        assert!(locked.is_none());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].map_address, 0x010D_856C);
    }

    #[test]
    fn klaipeda_style_name_hp_without_level_map_does_not_publish() {
        let name = 0x0177_AE00;
        let hp = 0x0177_8190;
        assert_eq!(name - hp, 0x2C70);
        let (locked, candidates) = resolve_presence_memory_profiles(
            None,
            PresenceAddressOverrides {
                name: Some(name),
                hp: Some(hp),
                ..PresenceAddressOverrides::default()
            },
        );
        assert!(locked.is_none());
        assert!(candidates.is_empty());
        assert!(explicit_presence_profile(PresenceAddressOverrides {
            name: Some(name),
            hp: Some(hp),
            ..PresenceAddressOverrides::default()
        })
        .is_none());
    }

    #[test]
    fn explicit_name_level_map_builds_one_profile_that_can_go_ingame() {
        let overrides = PresenceAddressOverrides {
            name: Some(0x0177_AE00),
            hp: Some(0x0177_8190),
            level: Some(0x0177_8000),
            job: Some(0x0177_8008),
            map: Some(0x0177_7000),
        };
        let (locked, candidates) = resolve_presence_memory_profiles(None, overrides);
        assert!(locked.is_none());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "scanned");
        assert_eq!(candidates[0].name_address, 0x0177_AE00);
        assert_eq!(candidates[0].level_address, 0x0177_8000);
        assert_eq!(candidates[0].job_level_address, 0x0177_8008);
        assert_eq!(candidates[0].map_address, 0x0177_7000);

        let memory = FakeMemory {
            u32_values: HashMap::from([(0x0177_8000, 12), (0x0177_8008, 8)]),
            strings: HashMap::from([
                (0x0177_AE00, "Klaipeda".into()),
                (0x0177_7000, "malaya.rsw".into()),
            ]),
        };
        let snapshot = read_character_snapshot(&memory, &candidates[0]).unwrap();
        assert_eq!(snapshot.state, CharacterState::InGame);
        assert_eq!(snapshot.character_name.as_deref(), Some("Klaipeda"));
        assert_eq!(snapshot.level, Some(12));
        assert_eq!(snapshot.map_name.as_deref(), Some("malaya"));
    }

    #[test]
    fn map_refine_discards_secondary_buffers() {
        assert!(map_label_matches("prontera.rsw", "prontera"));
        assert!(map_label_matches("PRONTERA", "prontera"));
        assert!(!map_label_matches("izlude.rsw", "prontera"));

        let needles = map_scan_needles("Prontera.rsw").expect("map");
        assert!(needles.iter().any(|needle| needle == b"prontera\0"));
        assert!(needles.iter().any(|needle| needle == b"prontera.rsw\0"));

        let leftovers: Vec<u32> = [
            (0x1000u32, "prontera.rsw"),
            (0x2000, "prontera"),
            (0x3000, "izlude.rsw"),
        ]
        .into_iter()
        .filter(|(_, raw)| map_label_matches(raw, "izlude"))
        .map(|(address, _)| address)
        .collect();
        assert_eq!(leftovers, vec![0x3000]);
    }
}
