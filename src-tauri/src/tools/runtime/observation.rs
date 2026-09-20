use serde::{Deserialize, Serialize};

use crate::tools::runner_sessions::ProcessExit;
use crate::utils::app_data_dir;
use crate::utils::process::redact_sensitive_values;

use super::host_gpu::HostGpuObservationIpc;

pub(crate) const RUNTIME_OBSERVATION_SCHEMA_VERSION: u32 = 1;

pub(crate) fn runtime_observe_enabled() -> bool {
    runtime_observe_enabled_from(std::env::var("RO_LAUNCHER_RUNTIME_OBSERVE").ok().as_deref())
}

pub(crate) fn runtime_observe_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum RunOutcome {
    #[serde(rename_all = "camelCase")]
    StartupFailed {
        class: StartupFailureClass,
    },
    StartupTimeout,
    #[serde(rename_all = "camelCase")]
    UserStop {
        controller_exit_code: i32,
    },
    #[serde(rename_all = "camelCase")]
    CleanExit {
        controller_exit_code: i32,
    },
    #[serde(rename_all = "camelCase")]
    CrashConfirmed {
        evidence: CrashEvidence,
    },
    #[serde(rename_all = "camelCase")]
    AbnormalControllerExit {
        controller_exit_code: i32,
    },
    #[serde(rename_all = "camelCase")]
    ProcessEnded {
        controller_exit_code: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StartupFailureClass {
    ControllerIdentityMissing,
    GameProcessWaitFailed,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum CrashEvidence {
    #[serde(rename_all = "camelCase")]
    ControllerSignal { signal: i32 },
    #[serde(rename_all = "camelCase")]
    NtStatus { code: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum VisualCheckStatus {
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RecordState {
    Started,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PlanAvailability {
    Resolved,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutcomeInput {
    pub reached_running: bool,
    pub stop_requested: bool,
    pub startup_timeout: bool,
    pub startup_failure: Option<StartupFailureClass>,
    pub controller_exit_before_terminate: Option<ProcessExit>,
    pub controller_exit_after_terminate: i32,
}

pub(crate) fn classify_run_outcome(input: OutcomeInput) -> RunOutcome {
    if input.stop_requested {
        return RunOutcome::UserStop {
            controller_exit_code: input.controller_exit_after_terminate,
        };
    }
    if !input.reached_running && input.startup_timeout {
        return RunOutcome::StartupTimeout;
    }
    if !input.reached_running {
        return RunOutcome::StartupFailed {
            class: input.startup_failure.unwrap_or(StartupFailureClass::Other),
        };
    }
    if let Some(exit) = input.controller_exit_before_terminate {
        if let Some(signal) = exit.signal {
            if matches!(signal, 4 | 5 | 6 | 7 | 8 | 11) {
                return RunOutcome::CrashConfirmed {
                    evidence: CrashEvidence::ControllerSignal { signal },
                };
            }
        }
        if let Some(code) = exit.exit_code {
            let status = code as u32;
            if matches!(status, 0xC000_0005 | 0xC000_00FD | 0xC000_0374) {
                return RunOutcome::CrashConfirmed {
                    evidence: CrashEvidence::NtStatus { code: status },
                };
            }
            if code == 0 && exit.signal.is_none() {
                return RunOutcome::CleanExit {
                    controller_exit_code: input.controller_exit_after_terminate,
                };
            }
            return RunOutcome::AbnormalControllerExit {
                controller_exit_code: code,
            };
        }
    }
    RunOutcome::ProcessEnded {
        controller_exit_code: input.controller_exit_after_terminate,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FingerprintEnvelopeIpc {
    pub schema_version: u64,
    pub algorithm: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum ObservationSubjectIpc {
    Absent,
    Unreadable,
    #[serde(rename_all = "camelCase")]
    Gepard {
        sha256: String,
        file_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservationReceiptIpc {
    pub artifact_id: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessIdentityIpc {
    pub pid: u32,
    pub start_time: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservationProcessIpc {
    pub game: Option<ProcessIdentityIpc>,
    pub controller: Option<ProcessIdentityIpc>,
    pub final_game: Option<ProcessIdentityIpc>,
    pub identity_stale: bool,
    pub handoff_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservationSupervisorIpc {
    pub enabled: bool,
    pub supervisor: Option<ProcessIdentityIpc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservationAppImageIpc {
    pub appdir_set: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservationFlagsIpc {
    pub observe: bool,
    pub graphics: bool,
    pub compat: bool,
    pub shadow: bool,
    pub prefix_v3: bool,
    pub session_supervisor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeObservationV1 {
    pub schema_version: u32,
    pub observation_id: String,
    pub record_state: RecordState,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub client_id: String,
    pub server_local_id: String,
    pub server_token: String,
    pub game_executable_name: String,
    pub invocation_target: String,
    pub plan_id: String,
    pub runtime_fingerprint: FingerprintEnvelopeIpc,
    pub prefix_fingerprint: FingerprintEnvelopeIpc,
    pub prefix_token: String,
    pub selection_source: String,
    pub runner_kind: String,
    pub graphics_profile: String,
    pub dxvk_provider: String,
    pub dxvk_component_id: String,
    pub overlay_verified: bool,
    pub receipts: Vec<ObservationReceiptIpc>,
    pub subject: ObservationSubjectIpc,
    pub host_gpu: HostGpuObservationIpc,
    pub process: ObservationProcessIpc,
    pub supervisor: ObservationSupervisorIpc,
    pub appimage: ObservationAppImageIpc,
    pub flags: ObservationFlagsIpc,
    pub outcome: Option<RunOutcome>,
    pub visual_check: VisualCheckStatus,
    pub plan_availability: PlanAvailability,
}

pub(crate) fn redact_observation_for_disk(record: &RuntimeObservationV1) -> RuntimeObservationV1 {
    let home = std::env::var("HOME").unwrap_or_default();
    let app_data = app_data_dir().to_string_lossy().to_string();
    let secrets = vec![home, app_data]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let json = serde_json::to_value(record).expect("observation json");
    let redacted = redact_json_value(&json, &secrets);
    serde_json::from_value(redacted).expect("observation roundtrip")
}

fn redact_json_value(value: &serde_json::Value, secrets: &[String]) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(redact_sensitive_values(text, secrets))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| redact_json_value(item, secrets))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map {
                out.insert(key.clone(), redact_json_value(item, secrets));
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_observe_flag_semantics() {
        assert!(runtime_observe_enabled_from(None));
        assert!(runtime_observe_enabled_from(Some("1")));
        assert!(!runtime_observe_enabled_from(Some("0")));
    }

    #[test]
    fn user_stop_beats_crash_signal() {
        let outcome = classify_run_outcome(OutcomeInput {
            reached_running: true,
            stop_requested: true,
            startup_timeout: false,
            startup_failure: None,
            controller_exit_before_terminate: Some(ProcessExit {
                exit_code: None,
                signal: Some(11),
            }),
            controller_exit_after_terminate: 0,
        });
        assert!(matches!(outcome, RunOutcome::UserStop { .. }));
    }

    #[test]
    fn startup_timeout_is_not_crash() {
        let outcome = classify_run_outcome(OutcomeInput {
            reached_running: false,
            stop_requested: false,
            startup_timeout: true,
            startup_failure: None,
            controller_exit_before_terminate: None,
            controller_exit_after_terminate: -1,
        });
        assert!(matches!(outcome, RunOutcome::StartupTimeout));
    }

    #[test]
    fn redaction_strips_home_from_strings() {
        let home = "/tmp/ro-launcher-redact-home";
        std::env::set_var("HOME", home);
        let record = RuntimeObservationV1 {
            schema_version: 1,
            observation_id: "obs-test".to_string(),
            record_state: RecordState::Started,
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: None,
            client_id: "client".to_string(),
            server_local_id: "server".to_string(),
            server_token: "token".to_string(),
            game_executable_name: "ragexe.exe".to_string(),
            invocation_target: "game".to_string(),
            plan_id: "plan".to_string(),
            runtime_fingerprint: FingerprintEnvelopeIpc {
                schema_version: 1,
                algorithm: "sha256".to_string(),
                digest: "a".repeat(64),
            },
            prefix_fingerprint: FingerprintEnvelopeIpc {
                schema_version: 1,
                algorithm: "sha256".to_string(),
                digest: "b".repeat(64),
            },
            prefix_token: "prefix".to_string(),
            selection_source: "productDefault".to_string(),
            runner_kind: "proton".to_string(),
            graphics_profile: "dxvk".to_string(),
            dxvk_provider: "runnerOwned".to_string(),
            dxvk_component_id: "runner/dxvk".to_string(),
            overlay_verified: false,
            receipts: Vec::new(),
            subject: ObservationSubjectIpc::Absent,
            host_gpu: HostGpuObservationIpc {
                completeness: "unavailable".to_string(),
                cards: Vec::new(),
                vulkan_api: None,
            },
            process: ObservationProcessIpc {
                game: None,
                controller: None,
                final_game: None,
                identity_stale: false,
                handoff_count: 0,
            },
            supervisor: ObservationSupervisorIpc {
                enabled: true,
                supervisor: None,
            },
            appimage: ObservationAppImageIpc { appdir_set: false },
            flags: ObservationFlagsIpc {
                observe: true,
                graphics: true,
                compat: true,
                shadow: true,
                prefix_v3: true,
                session_supervisor: true,
            },
            outcome: None,
            visual_check: VisualCheckStatus::Pending,
            plan_availability: PlanAvailability::Resolved,
        };
        let mut poisoned = record;
        poisoned.prefix_token = format!("{home}/prefixes/foo");
        let redacted = redact_observation_for_disk(&poisoned);
        assert!(!redacted.prefix_token.contains(home));
    }

    #[test]
    fn runtime_observation_fixture_roundtrip() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../contract-fixtures/runtime-observation-v1.json");
        let raw = std::fs::read_to_string(path).expect("fixture");
        let expected: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let parsed: RuntimeObservationV1 = serde_json::from_value(expected.clone()).unwrap();
        let actual = serde_json::to_value(&parsed).unwrap();
        assert_eq!(actual, expected);
    }
}
