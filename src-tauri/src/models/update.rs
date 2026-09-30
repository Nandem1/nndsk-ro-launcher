use serde::{Deserialize, Serialize};

pub const MAX_RELEASE_NOTES_CHARS: usize = 480;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSnapshot {
    pub current_version: String,
    pub phase: UpdatePhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdatePhase {
    Idle,
    Checking,
    Current,
    Available {
        release: ReleaseSummary,
    },
    Downloading {
        release: ReleaseSummary,
        downloaded_bytes: u64,
        content_length: Option<u64>,
    },
    ReadyToRestart {
        installed_version: String,
    },
    Failed {
        class: UpdateFailureClass,
        message: String,
    },
    Unavailable {
        reason: UpdateUnavailableReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummary {
    pub version: String,
    pub published_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UpdateFailureClass {
    Unauthentic,
    Unreachable,
    Install,
    ClientsActive,
    NoPending,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UpdateUnavailableReason {
    NotPackaged,
    UnsupportedOs,
}

pub fn truncate_release_notes(raw: Option<&str>) -> Option<String> {
    let text = raw?.trim();
    if text.is_empty() {
        return None;
    }
    let plain: String = text
        .chars()
        .map(|ch| {
            if ch.is_control() && ch != '\n' {
                ' '
            } else {
                ch
            }
        })
        .collect();
    let plain = plain.trim();
    if plain.is_empty() {
        return None;
    }
    if plain.chars().count() <= MAX_RELEASE_NOTES_CHARS {
        return Some(plain.to_string());
    }
    let truncated: String = plain.chars().take(MAX_RELEASE_NOTES_CHARS).collect();
    Some(format!("{truncated}…"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_notes_and_strips_control_chars() {
        let long = "a".repeat(MAX_RELEASE_NOTES_CHARS + 20);
        let notes = truncate_release_notes(Some(&long)).unwrap();
        assert!(notes.ends_with('…'));
        assert_eq!(notes.chars().count(), MAX_RELEASE_NOTES_CHARS + 1);

        assert_eq!(
            truncate_release_notes(Some("hola\u{0007}mundo")),
            Some("hola mundo".to_string())
        );
        assert_eq!(truncate_release_notes(Some("   ")), None);
    }
}
