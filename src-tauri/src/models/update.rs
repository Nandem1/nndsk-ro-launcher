use serde::{Deserialize, Serialize};

pub const MAX_RELEASE_NOTES_CHARS: usize = 480;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSnapshot {
    pub current_version: String,
    pub phase: UpdatePhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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

    #[test]
    fn serializes_phase_fields_as_camel_case() {
        let ready = UpdateSnapshot {
            current_version: "0.1.0".into(),
            phase: UpdatePhase::ReadyToRestart {
                installed_version: "0.2.0".into(),
            },
        };
        assert_eq!(
            serde_json::to_value(&ready).unwrap(),
            serde_json::json!({
                "currentVersion": "0.1.0",
                "phase": {
                    "kind": "readyToRestart",
                    "installedVersion": "0.2.0"
                }
            })
        );

        let downloading = UpdateSnapshot {
            current_version: "0.1.0".into(),
            phase: UpdatePhase::Downloading {
                release: ReleaseSummary {
                    version: "0.2.0".into(),
                    published_at: Some("2026-09-30T00:00:00Z".into()),
                    notes: None,
                },
                downloaded_bytes: 25,
                content_length: Some(100),
            },
        };
        let phase = serde_json::to_value(&downloading)
            .unwrap()
            .get("phase")
            .cloned()
            .unwrap();
        assert_eq!(phase.get("downloadedBytes"), Some(&serde_json::json!(25)));
        assert_eq!(phase.get("contentLength"), Some(&serde_json::json!(100)));
        assert_eq!(
            phase
                .get("release")
                .and_then(|release| release.get("publishedAt")),
            Some(&serde_json::json!("2026-09-30T00:00:00Z"))
        );
        assert_eq!(phase.get("downloaded_bytes"), None);
        assert_eq!(phase.get("content_length"), None);
    }
}
