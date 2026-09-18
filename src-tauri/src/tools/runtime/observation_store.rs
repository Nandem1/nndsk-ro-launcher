use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use ro_tools_linux::ProcessIdentity;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeObservationExportV1 {
    observation_id: String,
    record_state: String,
    started_at: String,
    finished_at: Option<String>,
    client_token: String,
    server_token: String,
    plan_id: String,
    outcome_kind: Option<String>,
    runtime_fingerprint: FingerprintEnvelopeIpc,
    prefix_fingerprint: FingerprintEnvelopeIpc,
    runner_kind: String,
    graphics_profile: String,
    host_gpu: super::host_gpu::HostGpuObservationIpc,
    outcome: Option<RunOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeObservationExportBundle {
    schema_version: u32,
    exported_at: String,
    observations: Vec<RuntimeObservationExportV1>,
}

use crate::models::observation::RuntimeObservationSummaryIpc;
use crate::tools::runner_sessions::session_supervisor_enabled;
use crate::utils::{app_data_dir, replace_json};

use super::apply::{build_runtime_plan_summary, runtime_graphics_plan_enabled};
use super::compatibility::runtime_compat_enabled;
use super::encode::{server_path_token16, sha256_hex};
use super::fingerprint::RuntimeFingerprint;
use super::identity::prefix_v3_write_enabled;
use super::inspection::inspect_subject;
use super::model::RuntimePlan;
use super::observation::{
    redact_observation_for_disk, runtime_observe_enabled, FingerprintEnvelopeIpc,
    ObservationAppImageIpc, ObservationFlagsIpc, ObservationProcessIpc, ObservationReceiptIpc,
    ObservationSubjectIpc, ObservationSupervisorIpc, PlanAvailability, ProcessIdentityIpc,
    RecordState, RunOutcome, RuntimeObservationV1, VisualCheckStatus,
    RUNTIME_OBSERVATION_SCHEMA_VERSION,
};
use super::shadow::runtime_shadow_enabled;
use super::RuntimeProfile;
use crate::utils::RunnerKind;

static OBSERVATIONS_LOCK: Mutex<()> = Mutex::new(());

const MAX_RECORDS: usize = 200;
const MAX_AGE: Duration = Duration::from_secs(90 * 24 * 3600);

#[derive(Debug, Clone)]
pub(crate) struct ObservationPaths {
    pub root: PathBuf,
}

impl ObservationPaths {
    pub(crate) fn production() -> Self {
        Self {
            root: app_data_dir().join("observations"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ObservationStartedPayload {
    pub observation_id: String,
    pub client_id: String,
    pub server_local_id: String,
    pub game_executable_name: String,
    pub invocation_target: String,
    pub plan_id: String,
    pub runtime_fingerprint: RuntimeFingerprint,
    pub prefix_fingerprint_hex: String,
    pub prefix_token: String,
    pub profile: RuntimeProfile,
    pub plan: RuntimePlan,
    pub overlay_verified: bool,
    pub game_dir: Option<String>,
    pub game_identity: Option<ProcessIdentity>,
    pub controller_identity: Option<ProcessIdentity>,
    pub supervisor_identity: Option<ProcessIdentity>,
    pub supervised: bool,
    pub plan_availability: PlanAvailability,
}

#[derive(Debug, Clone)]
pub(crate) struct ObservationFinishedPayload {
    pub observation_id: String,
    pub outcome: RunOutcome,
    pub game_identity: Option<ProcessIdentity>,
    pub controller_identity: Option<ProcessIdentity>,
    pub identity_stale: bool,
    pub handoff_count: u32,
}

pub(crate) fn enqueue_persist_started(payload: ObservationStartedPayload) {
    if !runtime_observe_enabled() {
        return;
    }
    let paths = ObservationPaths::production();
    tokio::task::spawn_blocking(move || {
        let _ = persist_started(&paths, payload);
    });
}

pub(crate) fn enqueue_persist_finished(payload: ObservationFinishedPayload) {
    if !runtime_observe_enabled() {
        return;
    }
    let paths = ObservationPaths::production();
    tokio::task::spawn_blocking(move || {
        let _ = persist_finished(&paths, payload);
    });
}

pub(crate) fn enqueue_persist_unreached(
    started: ObservationStartedPayload,
    finished: ObservationFinishedPayload,
) {
    if !runtime_observe_enabled() {
        return;
    }
    let paths = ObservationPaths::production();
    tokio::task::spawn_blocking(move || {
        if persist_started(&paths, started).is_ok() {
            let _ = persist_finished(&paths, finished);
        }
    });
}

fn persist_started(
    paths: &ObservationPaths,
    payload: ObservationStartedPayload,
) -> Result<(), String> {
    let _guard = OBSERVATIONS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let record = build_started_record(&payload)?;
    write_record(paths, &record)?;
    prune(paths, &record.observation_id)?;
    Ok(())
}

fn persist_finished(
    paths: &ObservationPaths,
    payload: ObservationFinishedPayload,
) -> Result<(), String> {
    let _guard = OBSERVATIONS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = record_path(paths, &payload.observation_id);
    let mut record = if path.exists() {
        read_record(&path)?
    } else {
        return Err("observation-started-missing".into());
    };
    if record.record_state == RecordState::Finished {
        return Ok(());
    }
    record.record_state = RecordState::Finished;
    record.finished_at = Some(Utc::now().to_rfc3339());
    record.outcome = Some(payload.outcome);
    record.process.game = payload.game_identity.map(to_identity_ipc);
    record.process.final_game = payload.game_identity.map(to_identity_ipc);
    record.process.controller = payload.controller_identity.map(to_identity_ipc);
    record.process.identity_stale = payload.identity_stale;
    record.process.handoff_count = payload.handoff_count;
    write_record(paths, &record)?;
    prune(paths, &record.observation_id)?;
    Ok(())
}

fn build_started_record(
    payload: &ObservationStartedPayload,
) -> Result<RuntimeObservationV1, String> {
    let summary = build_runtime_plan_summary(
        payload.plan_id.clone(),
        &payload.profile,
        &payload.plan,
        payload.overlay_verified,
    );
    let subject = map_subject(inspect_subject(
        payload.game_dir.as_deref().map(std::path::Path::new),
    ));
    let record = RuntimeObservationV1 {
        schema_version: RUNTIME_OBSERVATION_SCHEMA_VERSION,
        observation_id: payload.observation_id.clone(),
        record_state: RecordState::Started,
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        client_id: payload.client_id.clone(),
        server_local_id: payload.server_local_id.clone(),
        server_token: server_path_token16(&payload.server_local_id),
        game_executable_name: payload.game_executable_name.clone(),
        invocation_target: payload.invocation_target.clone(),
        plan_id: payload.plan_id.clone(),
        runtime_fingerprint: envelope_from_runtime(&payload.runtime_fingerprint),
        prefix_fingerprint: FingerprintEnvelopeIpc {
            schema_version: 1,
            algorithm: "sha256".to_string(),
            digest: payload.prefix_fingerprint_hex.clone(),
        },
        prefix_token: payload.prefix_token.clone(),
        selection_source: summary.selection_source.to_string(),
        runner_kind: match payload.plan.runner().resolved().kind() {
            RunnerKind::Wine => "wine".to_string(),
            RunnerKind::Proton => "proton".to_string(),
        },
        graphics_profile: summary.graphics_profile.to_string(),
        dxvk_provider: summary.dxvk_provider.to_string(),
        dxvk_component_id: summary.dxvk_component_id.clone(),
        overlay_verified: payload.overlay_verified,
        receipts: observation_receipts(&payload.plan),
        subject,
        host_gpu: super::host_gpu::probe_host_gpu(),
        process: ObservationProcessIpc {
            game: payload.game_identity.map(to_identity_ipc),
            controller: payload.controller_identity.map(to_identity_ipc),
            final_game: None,
            identity_stale: false,
            handoff_count: 0,
        },
        supervisor: ObservationSupervisorIpc {
            enabled: payload.supervised,
            supervisor: payload.supervisor_identity.map(to_identity_ipc),
        },
        appimage: ObservationAppImageIpc {
            appdir_set: std::env::var_os("APPDIR").is_some(),
        },
        flags: ObservationFlagsIpc {
            observe: runtime_observe_enabled(),
            graphics: runtime_graphics_plan_enabled(),
            compat: runtime_compat_enabled(),
            shadow: runtime_shadow_enabled(),
            prefix_v3: prefix_v3_write_enabled(),
            session_supervisor: session_supervisor_enabled(),
        },
        outcome: None,
        visual_check: VisualCheckStatus::Pending,
        plan_availability: payload.plan_availability,
    };
    Ok(record)
}

fn map_subject(subject: super::compatibility::SubjectObservation) -> ObservationSubjectIpc {
    match subject {
        super::compatibility::SubjectObservation::Absent => ObservationSubjectIpc::Absent,
        super::compatibility::SubjectObservation::Unreadable => ObservationSubjectIpc::Unreadable,
        super::compatibility::SubjectObservation::Hash { sha256_lowercase } => {
            ObservationSubjectIpc::Gepard {
                sha256: sha256_lowercase,
                file_name: "gepard.dll".to_string(),
            }
        }
    }
}

fn observation_receipts(plan: &RuntimePlan) -> Vec<ObservationReceiptIpc> {
    let mut receipts = Vec::new();
    if let Some(artifact_id) = plan.graphics().dxvk_provider().managed_dxvk_artifact_id() {
        receipts.push(ObservationReceiptIpc {
            artifact_id: artifact_id.to_string(),
            source: "artifact-receipt".to_string(),
        });
    }
    if let Some(overlay) = plan.graphics().overlay() {
        receipts.push(ObservationReceiptIpc {
            artifact_id: overlay.component_id.as_str().to_string(),
            source: "bundled-resource".to_string(),
        });
    }
    receipts
}

fn envelope_from_runtime(fp: &RuntimeFingerprint) -> FingerprintEnvelopeIpc {
    FingerprintEnvelopeIpc {
        schema_version: fp.digest.schema_version,
        algorithm: fp.digest.algorithm.clone(),
        digest: fp.digest.hex_digest(),
    }
}

fn to_identity_ipc(identity: ProcessIdentity) -> ProcessIdentityIpc {
    ProcessIdentityIpc {
        pid: identity.pid,
        start_time: identity.start_time,
    }
}

fn record_path(paths: &ObservationPaths, observation_id: &str) -> PathBuf {
    paths.root.join(format!("obs-{observation_id}.json"))
}

fn write_record(paths: &ObservationPaths, record: &RuntimeObservationV1) -> Result<(), String> {
    fs::create_dir_all(&paths.root).map_err(|error| error.to_string())?;
    let redacted = redact_observation_for_disk(record);
    replace_json(&record_path(paths, &record.observation_id), &redacted)
}

fn read_record(path: &Path) -> Result<RuntimeObservationV1, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let record: RuntimeObservationV1 = serde_json::from_str(&content)
        .map_err(|error| format!("observación inválida en {}: {error}", path.display()))?;
    if record.schema_version != RUNTIME_OBSERVATION_SCHEMA_VERSION {
        quarantine_corrupt(path)?;
        return Err("observation-schema-unsupported".into());
    }
    Ok(record)
}

fn prune(paths: &ObservationPaths, keep_id: &str) -> Result<(), String> {
    if !paths.root.is_dir() {
        return Ok(());
    }
    let now = SystemTime::now();
    let mut records = Vec::new();
    for entry in fs::read_dir(&paths.root)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("obs-") && name.ends_with(".json"))
        {
            continue;
        }
        match read_record(&path) {
            Ok(record) => records.push((path, record)),
            Err(_) => {
                let _ = quarantine_corrupt(&path);
            }
        }
    }
    for (path, record) in &records {
        if record.observation_id == keep_id {
            continue;
        }
        if let Ok(started) = chrono::DateTime::parse_from_rfc3339(&record.started_at) {
            let age = now
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(started.timestamp().max(0) as u64);
            if age > MAX_AGE.as_secs() {
                let _ = fs::remove_file(path);
            }
        }
    }
    records.retain(|(path, _)| path.exists());
    if records.len() <= MAX_RECORDS {
        return Ok(());
    }
    records.sort_by(|left, right| {
        left.1
            .started_at
            .cmp(&right.1.started_at)
            .then_with(|| left.1.observation_id.cmp(&right.1.observation_id))
    });
    let excess = records.len() - MAX_RECORDS;
    for (path, record) in records.into_iter().take(excess) {
        if record.observation_id == keep_id {
            continue;
        }
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub(crate) fn list_observations() -> Result<Vec<RuntimeObservationSummaryIpc>, String> {
    let paths = ObservationPaths::production();
    let _guard = OBSERVATIONS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !paths.root.is_dir() {
        return Ok(Vec::new());
    }
    let mut summaries = Vec::new();
    for entry in fs::read_dir(&paths.root)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let path = entry.path();
        match read_record(&path) {
            Ok(record) => summaries.push(RuntimeObservationSummaryIpc {
                observation_id: record.observation_id.clone(),
                record_state: record_state_label(record.record_state),
                started_at: record.started_at.clone(),
                finished_at: record.finished_at.clone(),
                server_token: record.server_token.clone(),
                plan_id: record.plan_id.clone(),
                outcome_kind: record.outcome.as_ref().map(outcome_kind_label),
                visual_check: "pending".to_string(),
            }),
            Err(_) => continue,
        }
    }
    summaries.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    Ok(summaries)
}

pub(crate) fn delete_observations() -> Result<(), String> {
    let paths = ObservationPaths::production();
    let _guard = OBSERVATIONS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if paths.root.is_dir() {
        fs::remove_dir_all(&paths.root).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&paths.root).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn export_observations(dest_path: &Path) -> Result<usize, String> {
    let app_data = app_data_dir();
    if dest_path.is_relative() || dest_path.starts_with(&app_data) {
        return Err("export-destination-rejected".into());
    }
    let paths = ObservationPaths::production();
    let _guard = OBSERVATIONS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut exported = Vec::new();
    if paths.root.is_dir() {
        for entry in fs::read_dir(&paths.root)
            .map_err(|error| error.to_string())?
            .flatten()
        {
            let path = entry.path();
            if let Ok(record) = read_record(&path) {
                exported.push(export_record(&record));
            }
        }
    }
    let bundle = RuntimeObservationExportBundle {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        observations: exported,
    };
    let count = bundle.observations.len();
    replace_json(dest_path, &bundle)?;
    Ok(count)
}

fn export_record(record: &RuntimeObservationV1) -> RuntimeObservationExportV1 {
    let client_token = sha256_hex(record.client_id.as_bytes())[..16].to_string();
    RuntimeObservationExportV1 {
        observation_id: record.observation_id.clone(),
        record_state: record_state_label(record.record_state),
        started_at: record.started_at.clone(),
        finished_at: record.finished_at.clone(),
        client_token,
        server_token: record.server_token.clone(),
        plan_id: record.plan_id.clone(),
        outcome_kind: record.outcome.as_ref().map(outcome_kind_label),
        runtime_fingerprint: record.runtime_fingerprint.clone(),
        prefix_fingerprint: record.prefix_fingerprint.clone(),
        runner_kind: record.runner_kind.clone(),
        graphics_profile: record.graphics_profile.clone(),
        host_gpu: record.host_gpu.clone(),
        outcome: record.outcome,
    }
}

fn record_state_label(state: RecordState) -> String {
    match state {
        RecordState::Started => "started".to_string(),
        RecordState::Finished => "finished".to_string(),
    }
}

fn outcome_kind_label(outcome: &RunOutcome) -> String {
    match outcome {
        RunOutcome::StartupFailed { .. } => "startupFailed".to_string(),
        RunOutcome::StartupTimeout => "startupTimeout".to_string(),
        RunOutcome::UserStop { .. } => "userStop".to_string(),
        RunOutcome::CleanExit { .. } => "cleanExit".to_string(),
        RunOutcome::CrashConfirmed { .. } => "crashConfirmed".to_string(),
        RunOutcome::AbnormalControllerExit { .. } => "abnormalControllerExit".to_string(),
        RunOutcome::ProcessEnded { .. } => "processEnded".to_string(),
    }
}

pub(crate) fn new_observation_id() -> String {
    Uuid::new_v4().to_string()
}

fn quarantine_corrupt(path: &Path) -> Result<(), String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let corrupt = path.with_file_name(format!(
        "{}.corrupt-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("observation.json"),
        timestamp
    ));
    fs::rename(path, corrupt).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static OBS_TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_observation_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ro-obs-store-{}-{}",
            std::process::id(),
            OBS_TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn unsupported_schema_is_quarantined() {
        let root = temp_observation_root();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("obs-bad-schema.json");
        fs::write(
            &path,
            r#"{"schemaVersion":2,"observationId":"x","recordState":"started"}"#,
        )
        .unwrap();
        assert!(read_record(&path).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_record_persists_valid_observation() {
        let root = temp_observation_root();
        let paths = ObservationPaths { root: root.clone() };
        fs::create_dir_all(&root).unwrap();
        let record = RuntimeObservationV1 {
            schema_version: RUNTIME_OBSERVATION_SCHEMA_VERSION,
            observation_id: "good".to_string(),
            record_state: RecordState::Started,
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: None,
            client_id: "c".to_string(),
            server_local_id: "s".to_string(),
            server_token: "t".to_string(),
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
            host_gpu: crate::tools::runtime::host_gpu::HostGpuObservationIpc {
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
                enabled: false,
                supervisor: None,
            },
            appimage: ObservationAppImageIpc { appdir_set: false },
            flags: ObservationFlagsIpc {
                observe: true,
                graphics: true,
                compat: true,
                shadow: false,
                prefix_v3: true,
                session_supervisor: true,
            },
            outcome: None,
            visual_check: VisualCheckStatus::Pending,
            plan_availability: PlanAvailability::Resolved,
        };
        write_record(&paths, &record).unwrap();
        let loaded = read_record(&record_path(&paths, &record.observation_id)).unwrap();
        assert_eq!(loaded.observation_id, "good");
        let _ = fs::remove_dir_all(root);
    }
}
