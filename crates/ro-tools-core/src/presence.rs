use std::time::Instant;

use serde::Deserialize;

use crate::error::ToolsError;
use crate::ports::MemoryReader;
use crate::profiles::parse_hex;

const MAX_TEXT_LEN: usize = 40;
const MAX_LEVEL: u32 = 300;

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

fn normalize_map_name(value: &str) -> Option<String> {
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
}
