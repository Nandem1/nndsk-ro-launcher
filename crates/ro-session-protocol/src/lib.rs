//! Shared NDJSON protocol types for RO-Launcher and ro-sessiond.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;
pub const MAX_LAUNCH_ARGS: usize = 4096;
pub const MAX_IN_FLIGHT_LAUNCHES: usize = 32;

pub const ERROR_STAGES: &[&str] = &[
    "handshake",
    "validation",
    "launch",
    "reap",
    "shutdown",
    "protocol",
    "internal",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: Vec<EnvironmentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentChange {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SessionRequest {
    Hello {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    Launch {
        request_id: String,
        spec: ProcessSpec,
    },
    Shutdown {
        request_id: String,
        shutdown_spec: ProcessSpec,
        grace_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SessionEvent {
    Ready {
        protocol_version: u16,
        supervisor_pid: u32,
        prefix: String,
        subreaper: bool,
    },
    LaunchAccepted {
        request_id: String,
        controller_pid: u32,
    },
    ShutdownAccepted {
        request_id: String,
    },
    ControllerExited {
        request_id: String,
        controller_pid: u32,
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    Idle,
    Error {
        request_id: Option<String>,
        stage: String,
        errno: Option<i32>,
        message: String,
    },
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    EmptyEnvKey,
    InvalidEnvKey,
    DuplicateEnvKey,
    EnvValueContainsNul,
    TooManyArgs,
    MissingWinePrefix,
    MultipleWinePrefix,
    WinePrefixUnset,
}

impl ValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::EmptyEnvKey => "env key must not be empty",
            Self::InvalidEnvKey => "env key must not contain NUL or '='",
            Self::DuplicateEnvKey => "duplicate env keys in ProcessSpec",
            Self::EnvValueContainsNul => "env value must not contain NUL",
            Self::TooManyArgs => "too many arguments in ProcessSpec",
            Self::MissingWinePrefix => "ProcessSpec must include WINEPREFIX",
            Self::MultipleWinePrefix => "ProcessSpec must include exactly one WINEPREFIX change",
            Self::WinePrefixUnset => "WINEPREFIX must be set, not unset",
        }
    }
}

pub fn validate_env_key(key: &str) -> Result<(), ValidationError> {
    if key.is_empty() {
        return Err(ValidationError::EmptyEnvKey);
    }
    if key.contains('\0') || key.contains('=') {
        return Err(ValidationError::InvalidEnvKey);
    }
    Ok(())
}

pub fn validate_process_spec_structure(spec: &ProcessSpec) -> Result<(), ValidationError> {
    if spec.args.len() > MAX_LAUNCH_ARGS {
        return Err(ValidationError::TooManyArgs);
    }
    let mut keys = HashSet::new();
    let mut wineprefix_count = 0usize;
    for change in &spec.env {
        validate_env_key(&change.key)?;
        if !keys.insert(change.key.clone()) {
            return Err(ValidationError::DuplicateEnvKey);
        }
        if let Some(ref value) = change.value {
            if value.contains('\0') {
                return Err(ValidationError::EnvValueContainsNul);
            }
        }
        if change.key == "WINEPREFIX" {
            wineprefix_count += 1;
            if change.value.is_none() {
                return Err(ValidationError::WinePrefixUnset);
            }
        }
    }
    if wineprefix_count == 0 {
        return Err(ValidationError::MissingWinePrefix);
    }
    if wineprefix_count > 1 {
        return Err(ValidationError::MultipleWinePrefix);
    }
    Ok(())
}

pub fn clamp_grace_ms(grace_ms: u64) -> u64 {
    grace_ms.clamp(1_000, 10_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> ProcessSpec {
        ProcessSpec {
            program: "/usr/bin/wine".into(),
            args: vec!["a.exe".into()],
            cwd: "/game".into(),
            env: vec![EnvironmentChange {
                key: "WINEPREFIX".into(),
                value: Some("/abs/prefix".into()),
            }],
        }
    }

    #[test]
    fn canonical_hello_roundtrip() {
        let req = SessionRequest::Hello {
            protocol_version: 1,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"type":"hello","protocolVersion":1}"#);
        let back: SessionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn canonical_ready_roundtrip() {
        let ev = SessionEvent::Ready {
            protocol_version: 1,
            supervisor_pid: 1234,
            prefix: "/abs/prefix".into(),
            subreaper: true,
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(
            json,
            r#"{"type":"ready","protocolVersion":1,"supervisorPid":1234,"prefix":"/abs/prefix","subreaper":true}"#
        );
        let back: SessionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn canonical_launch_example_from_plan() {
        let req = SessionRequest::Launch {
            request_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            spec: sample_spec(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(
            json,
            r#"{"type":"launch","requestId":"550e8400-e29b-41d4-a716-446655440000","spec":{"program":"/usr/bin/wine","args":["a.exe"],"cwd":"/game","env":[{"key":"WINEPREFIX","value":"/abs/prefix"}]}}"#
        );
        let back: SessionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn rejects_invalid_env_key() {
        assert!(validate_env_key("").is_err());
        assert!(validate_env_key("A=B").is_err());
        assert!(validate_env_key("bad\0key").is_err());
    }

    #[test]
    fn rejects_duplicate_and_missing_wineprefix() {
        let mut spec = sample_spec();
        spec.env.push(EnvironmentChange {
            key: "WINEPREFIX".into(),
            value: Some("/other".into()),
        });
        assert_eq!(
            validate_process_spec_structure(&spec),
            Err(ValidationError::DuplicateEnvKey)
        );

        spec.env.clear();
        assert_eq!(
            validate_process_spec_structure(&spec),
            Err(ValidationError::MissingWinePrefix)
        );
    }

    #[test]
    fn clamp_grace_ms_bounds() {
        assert_eq!(clamp_grace_ms(0), 1_000);
        assert_eq!(clamp_grace_ms(5_000), 5_000);
        assert_eq!(clamp_grace_ms(99_999), 10_000);
    }
}
