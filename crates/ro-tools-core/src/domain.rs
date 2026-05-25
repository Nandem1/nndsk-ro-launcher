use serde::{Deserialize, Serialize};

/// Memory layout shared by most rAthena / 4RTools-compatible clients.
///
/// Offsets match 4RTools `Client.cs`:
/// - max HP at base + 4
/// - current SP at base + 8
/// - max SP at base + 12
/// - status buffer at base + 0x474
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfile {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub exe_names: Vec<String>,
    /// HP base address (4RTools `hpAddress`), e.g. `0x10DCE10`
    pub hp_base: u32,
    /// Character name address (4RTools `nameAddress`), e.g. `0x10DF5D8`
    pub name_address: u32,
}

impl ClientProfile {
    pub fn max_hp_address(&self) -> u32 {
        self.hp_base + 4
    }

    pub fn cur_sp_address(&self) -> u32 {
        self.hp_base + 8
    }

    pub fn max_sp_address(&self) -> u32 {
        self.hp_base + 12
    }

    pub fn status_buffer_address(&self) -> u32 {
        self.hp_base + 0x474
    }

    pub fn matches_exe(&self, exe_name: &str) -> bool {
        let exe_lower = exe_name.to_ascii_lowercase();
        self.exe_names.iter().any(|pattern| {
            let pat = pattern.to_ascii_lowercase();
            if pat.contains('*') {
                glob_match(&pat, &exe_lower)
            } else {
                pat == exe_lower
            }
        })
    }
}

/// Default profile used by OsRO, HoneyRO and most private clients (4RTools OSRO entry).
pub fn default_profile() -> ClientProfile {
    ClientProfile {
        id: "rathena-default".into(),
        label: "rAthena / OSRO default".into(),
        exe_names: vec![
            "OsRO Midrate.exe".into(),
            "HoneyRO.exe".into(),
            "*RO*.exe".into(),
        ],
        hp_base: 0x10DCE10,
        name_address: 0x10DF5D8,
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern.eq_ignore_ascii_case(text);
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = text;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        let is_first = i == 0;
        let is_last = i == parts.len() - 1;

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

        let Some(pos) = rest.to_ascii_lowercase().find(&part.to_ascii_lowercase()) else {
            return false;
        };
        rest = &rest[pos + part.len()..];
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_match_4rtools_layout() {
        let p = default_profile();
        assert_eq!(p.max_hp_address(), 0x10DCE14);
        assert_eq!(p.cur_sp_address(), 0x10DCE18);
        assert_eq!(p.max_sp_address(), 0x10DCE1C);
        assert_eq!(p.status_buffer_address(), 0x10DD284);
        assert_eq!(p.name_address, 0x10DF5D8);
    }

    #[test]
    fn exe_glob_matching() {
        let p = default_profile();
        assert!(p.matches_exe("OsRO Midrate.exe"));
        assert!(p.matches_exe("HoneyRO.exe"));
        assert!(p.matches_exe("MyRO Client.exe"));
    }
}
