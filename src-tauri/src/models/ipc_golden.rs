//! Rust-produced golden corpus for tagged-enum DTOs that cross the IPC boundary.
//!
//! `contract-fixtures/ipc-tagged-enums-v1.json` is the single authority. This test proves Rust
//! serializes exactly that wire shape; `src/shared/ipcGolden.test.ts` proves the TypeScript
//! contract consumes the same file. Changing either side without the fixture fails a test.

use super::dependency::CompatibilityAssessmentIpc;
use super::environment::{EnvironmentAction, HostFamily};
use super::update::{ReleaseSummary, UpdateFailureClass, UpdatePhase, UpdateUnavailableReason};
use serde::Serialize;
use serde_json::{json, Map, Value};

fn wire<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("serializable DTO")
}

fn release(published_at: Option<&str>, notes: Option<&str>) -> ReleaseSummary {
    ReleaseSummary {
        version: "0.3.4".to_string(),
        published_at: published_at.map(str::to_string),
        notes: notes.map(str::to_string),
    }
}

fn group(cases: Vec<(&str, Value)>) -> Value {
    Value::Object(
        cases
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect::<Map<String, Value>>(),
    )
}

fn produced_corpus() -> Value {
    let compatibility = group(vec![
        (
            "validated",
            wire(&CompatibilityAssessmentIpc::Validated {
                evidence_id: "evidence-validated-1".to_string(),
            }),
        ),
        (
            "experimental",
            wire(&CompatibilityAssessmentIpc::Experimental {
                evidence_id: "evidence-experimental-1".to_string(),
            }),
        ),
        (
            "incompatible",
            wire(&CompatibilityAssessmentIpc::Incompatible {
                evidence_id: "evidence-incompatible-1".to_string(),
                reason: "known crash".to_string(),
            }),
        ),
        ("unknown", wire(&CompatibilityAssessmentIpc::Unknown)),
    ]);
    let environment = group(vec![
        ("recheck", wire(&EnvironmentAction::Recheck)),
        ("prepare", wire(&EnvironmentAction::Prepare)),
        (
            "hostGuide",
            wire(&EnvironmentAction::HostGuide {
                family: HostFamily::Arch,
                packages: vec!["wine".to_string(), "winetricks".to_string()],
            }),
        ),
    ]);
    let update = group(vec![
        ("idle", wire(&UpdatePhase::Idle)),
        ("checking", wire(&UpdatePhase::Checking)),
        ("current", wire(&UpdatePhase::Current)),
        (
            "available",
            wire(&UpdatePhase::Available {
                release: release(Some("2026-01-02T03:04:05Z"), Some("notes")),
            }),
        ),
        (
            "downloading",
            wire(&UpdatePhase::Downloading {
                release: release(None, None),
                downloaded_bytes: 1024,
                content_length: Some(4096),
            }),
        ),
        (
            "readyToRestart",
            wire(&UpdatePhase::ReadyToRestart {
                installed_version: "0.3.4".to_string(),
            }),
        ),
        (
            "failed",
            wire(&UpdatePhase::Failed {
                class: UpdateFailureClass::ClientsActive,
                message: "clients running".to_string(),
            }),
        ),
        (
            "unavailable",
            wire(&UpdatePhase::Unavailable {
                reason: UpdateUnavailableReason::NotPackaged,
            }),
        ),
    ]);
    json!({
        "compatibilityAssessment": compatibility,
        "environmentAction": environment,
        "updatePhase": update,
    })
}

#[test]
fn tagged_enum_wire_shapes_match_golden_corpus() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../contract-fixtures/ipc-tagged-enums-v1.json"
    ))
    .expect("golden corpus parses");
    assert_eq!(produced_corpus(), fixture);
}

#[test]
fn compatibility_assessment_uses_camel_case_evidence_id() {
    let validated = wire(&CompatibilityAssessmentIpc::Validated {
        evidence_id: "ev".to_string(),
    });
    assert_eq!(
        validated,
        json!({ "kind": "validated", "evidenceId": "ev" })
    );
    assert!(validated.get("evidence_id").is_none());
}
