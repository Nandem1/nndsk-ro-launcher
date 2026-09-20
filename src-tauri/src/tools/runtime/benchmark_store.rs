use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use ro_tools_linux::{verify_process_identity, ProcessIdentity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::benchmark::{
    BenchmarkComparisonIpc, BenchmarkExportResultIpc, BenchmarkRunSummaryIpc,
    CompareRuntimeBenchmarksIpc,
};
use crate::utils::{app_data_dir, replace_json};

use super::benchmark::{
    arm_label, compare_benchmark_runs, duration_within_tolerance, invalid_reason_label,
    linked_outcome_unusable, min_samples_for_capture, parse_imported_csv_v1, record_state_label,
    runtime_benchmark_enabled, visual_check_label, BenchmarkArm, BenchmarkInvalidReason,
    BenchmarkRecordState, BenchmarkVisualCheck, CaptureAdapterKind, RuntimeBenchmarkRunV1,
    RUNTIME_BENCHMARK_SCHEMA_VERSION,
};
use super::encode::sha256_hex;
use super::observation::ProcessIdentityIpc;

static BENCHMARKS_LOCK: Mutex<()> = Mutex::new(());

const MAX_RECORDS: usize = 100;
const MAX_AGE: Duration = Duration::from_secs(90 * 24 * 3600);

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkPaths {
    pub root: PathBuf,
}

impl BenchmarkPaths {
    pub(crate) fn production() -> Self {
        Self {
            root: app_data_dir().join("benchmarks"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBenchmarkExportV1 {
    run_id: String,
    record_state: String,
    invalid_reason: Option<String>,
    attached_at: String,
    capture_started_at: Option<String>,
    capture_finished_at: Option<String>,
    client_token: String,
    server_token: String,
    observation_id: Option<String>,
    arm: String,
    spec: super::benchmark::BenchmarkSpecV1,
    plan_id: String,
    runtime_fingerprint: super::observation::FingerprintEnvelopeIpc,
    prefix_fingerprint: super::observation::FingerprintEnvelopeIpc,
    prefix_token: String,
    selection_source: String,
    runner_kind: String,
    graphics_profile: String,
    dxvk_provider: String,
    dxvk_component_id: String,
    overlay_verified: bool,
    subject: super::observation::ObservationSubjectIpc,
    host_gpu: super::host_gpu::HostGpuObservationIpc,
    process: super::observation::ObservationProcessIpc,
    flags: super::benchmark::BenchmarkFlagsIpc,
    capture_adapter: CaptureAdapterKind,
    samples: Option<super::benchmark::BenchmarkSamplesV1>,
    visual_check: String,
    linked_outcome_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBenchmarkExportBundle {
    schema_version: u32,
    exported_at: String,
    runs: Vec<RuntimeBenchmarkExportV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBenchmarkComparisonExportBundle {
    schema_version: u32,
    exported_at: String,
    comparison: BenchmarkComparisonIpc,
}

fn record_path(paths: &BenchmarkPaths, run_id: &str) -> Result<PathBuf, String> {
    Uuid::parse_str(run_id).map_err(|_| "invalid-run-id".to_string())?;
    Ok(paths.root.join(format!("run-{run_id}.json")))
}

pub(crate) fn new_benchmark_run_id() -> String {
    Uuid::new_v4().to_string()
}

pub(crate) fn load_benchmark_run(run_id: &str) -> Result<RuntimeBenchmarkRunV1, String> {
    let paths = BenchmarkPaths::production();
    let path = record_path(&paths, run_id)?;
    if !path.exists() {
        return Err("run-not-found".into());
    }
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    read_record(&path)
}

pub(crate) fn attach_benchmark_run(
    record: RuntimeBenchmarkRunV1,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if client_has_active_capture(&paths, &record.client_id, None)? {
        return Err("client-capture-active".into());
    }
    write_record(&paths, &record)?;
    prune(&paths, &record.run_id)?;
    Ok(summary_from_record(&record))
}

pub(crate) fn begin_benchmark_capture(
    run_id: &str,
    identity: &ProcessIdentity,
    resolved_plan_id: &str,
    warmup_seconds: u32,
    process_age_secs: Option<f64>,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut record = read_record(&record_path(&paths, run_id)?)?;
    if record.record_state != BenchmarkRecordState::Attached {
        return Err("illegal-state-transition".into());
    }
    if !identity_matches_record(&record, identity) {
        invalidate(&mut record, BenchmarkInvalidReason::ProcessGone);
        write_record(&paths, &record)?;
        return Ok(summary_from_record(&record));
    }
    if record.plan_id != resolved_plan_id {
        invalidate(&mut record, BenchmarkInvalidReason::PlanChanged);
        write_record(&paths, &record)?;
        return Ok(summary_from_record(&record));
    }
    let age = process_age_secs.ok_or_else(|| "process-age-unavailable".to_string())?;
    if age < warmup_seconds as f64 {
        invalidate(&mut record, BenchmarkInvalidReason::WarmupTooShort);
        write_record(&paths, &record)?;
        return Ok(summary_from_record(&record));
    }
    record.record_state = BenchmarkRecordState::Capturing;
    record.capture_started_at = Some(Utc::now().to_rfc3339());
    write_record(&paths, &record)?;
    Ok(summary_from_record(&record))
}

pub(crate) fn finish_benchmark_capture(
    run_id: &str,
    identity: &ProcessIdentity,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut record = read_record(&record_path(&paths, run_id)?)?;
    if record.record_state != BenchmarkRecordState::Capturing {
        return Err("illegal-state-transition".into());
    }
    if !identity_matches_record(&record, identity) {
        invalidate(&mut record, BenchmarkInvalidReason::ProcessGone);
        write_record(&paths, &record)?;
        return Ok(summary_from_record(&record));
    }
    record.capture_finished_at = Some(Utc::now().to_rfc3339());
    let started = record.capture_started_at.clone().unwrap_or_default();
    let finished = record.capture_finished_at.clone().unwrap_or_default();
    let ok = duration_within_tolerance(&started, &finished, record.spec.capture_seconds)?;
    if ok {
        record.record_state = BenchmarkRecordState::CaptureFinished;
    } else {
        invalidate(&mut record, BenchmarkInvalidReason::DurationMismatch);
    }
    write_record(&paths, &record)?;
    Ok(summary_from_record(&record))
}

pub(crate) fn import_benchmark_samples(
    run_id: &str,
    csv_path: &Path,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut record = read_record(&record_path(&paths, run_id)?)?;
    if record.visual_check != BenchmarkVisualCheck::Pending {
        return Err("samples-frozen".into());
    }
    if !matches!(
        record.record_state,
        BenchmarkRecordState::CaptureFinished | BenchmarkRecordState::SamplesImported
    ) {
        return Err("illegal-state-transition".into());
    }
    let parsed = match parse_imported_csv_v1(csv_path) {
        Ok(parsed) => parsed,
        Err(error) if error == "csv-clock-not-monotonic" => {
            invalidate(&mut record, BenchmarkInvalidReason::ClockJumpOrPause);
            write_record(&paths, &record)?;
            return Ok(summary_from_record(&record));
        }
        Err(error) => return Err(error),
    };
    let min_samples = min_samples_for_capture(record.spec.capture_seconds);
    if parsed.frametime_ms.len() < min_samples {
        invalidate(&mut record, BenchmarkInvalidReason::InsufficientSamples);
        write_record(&paths, &record)?;
        return Ok(summary_from_record(&record));
    }
    record.samples = Some(super::benchmark::BenchmarkSamplesV1 {
        adapter: CaptureAdapterKind::ImportedCsvV1,
        source_sha256: parsed.source_sha256,
        sample_count: parsed.frametime_ms.len() as u32,
        frametime_ms: parsed.frametime_ms,
        monotonic_ns: parsed.monotonic_ns,
    });
    record.record_state = BenchmarkRecordState::SamplesImported;
    record.invalid_reason = None;
    write_record(&paths, &record)?;
    Ok(summary_from_record(&record))
}

pub(crate) fn set_benchmark_visual_check(
    run_id: &str,
    status: BenchmarkVisualCheck,
) -> Result<BenchmarkRunSummaryIpc, String> {
    if !runtime_benchmark_enabled() {
        return Err("runtime-benchmark-disabled".into());
    }
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut record = read_record(&record_path(&paths, run_id)?)?;
    if !matches!(
        record.record_state,
        BenchmarkRecordState::CaptureFinished | BenchmarkRecordState::SamplesImported
    ) {
        return Err("illegal-state-transition".into());
    }
    record.visual_check = status;
    write_record(&paths, &record)?;
    Ok(summary_from_record(&record))
}

pub(crate) fn list_benchmark_runs() -> Result<Vec<BenchmarkRunSummaryIpc>, String> {
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut summaries = Vec::new();
    if paths.root.is_dir() {
        for entry in fs::read_dir(&paths.root)
            .map_err(|error| error.to_string())?
            .flatten()
        {
            let path = entry.path();
            if let Ok(mut record) = read_record(&path) {
                if matches!(
                    record.record_state,
                    BenchmarkRecordState::Attached | BenchmarkRecordState::Capturing
                ) && !benchmark_process_is_alive(&record)
                {
                    invalidate(&mut record, BenchmarkInvalidReason::ProcessGone);
                    write_record(&paths, &record)?;
                }
                summaries.push(summary_from_record(&record));
            }
        }
    }
    summaries.sort_by(|left, right| right.attached_at.cmp(&left.attached_at));
    Ok(summaries)
}

pub(crate) fn delete_benchmark_runs() -> Result<(), String> {
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !paths.root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&paths.root)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("run-") && name.ends_with(".json"))
        {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn export_benchmark_runs(
    dest_path: &Path,
    run_ids: &[String],
) -> Result<BenchmarkExportResultIpc, String> {
    let app_data = app_data_dir();
    if dest_path.is_relative() || dest_path.starts_with(&app_data) {
        return Err("export-destination-rejected".into());
    }
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    validate_run_ids(run_ids)?;
    let mut runs = Vec::new();
    for run_id in run_ids {
        let record = read_record(&record_path(&paths, run_id)?)?;
        runs.push(export_record(&record));
    }
    let bundle = RuntimeBenchmarkExportBundle {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        runs,
    };
    replace_json(dest_path, &bundle)?;
    Ok(BenchmarkExportResultIpc {
        exported_count: bundle.runs.len(),
    })
}

pub(crate) fn compare_benchmark_runs_by_id(
    input: &CompareRuntimeBenchmarksIpc,
) -> Result<BenchmarkComparisonIpc, String> {
    let paths = BenchmarkPaths::production();
    let _guard = BENCHMARKS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if input.left_run_ids.is_empty() || input.right_run_ids.is_empty() {
        return Err("compare-empty-arm".into());
    }
    validate_comparison_ids(input)?;
    let mut left = Vec::new();
    for id in &input.left_run_ids {
        let path = record_path(&paths, id)?;
        if !path.exists() {
            return Err("run-not-found".into());
        }
        let record = read_record(&path)?;
        validate_for_compare(&record, BenchmarkArm::A)?;
        left.push(record);
    }
    let mut right = Vec::new();
    for id in &input.right_run_ids {
        let path = record_path(&paths, id)?;
        if !path.exists() {
            return Err("run-not-found".into());
        }
        let record = read_record(&path)?;
        validate_for_compare(&record, BenchmarkArm::B)?;
        right.push(record);
    }
    Ok(compare_benchmark_runs(left, right))
}

pub(crate) fn export_benchmark_comparison(
    dest_path: &Path,
    input: &CompareRuntimeBenchmarksIpc,
) -> Result<BenchmarkExportResultIpc, String> {
    let app_data = app_data_dir();
    if dest_path.is_relative() || dest_path.starts_with(&app_data) {
        return Err("export-destination-rejected".into());
    }
    let comparison = compare_benchmark_runs_by_id(input)?;
    let bundle = RuntimeBenchmarkComparisonExportBundle {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        comparison,
    };
    replace_json(dest_path, &bundle)?;
    Ok(BenchmarkExportResultIpc { exported_count: 1 })
}

fn validate_run_ids(run_ids: &[String]) -> Result<(), String> {
    if run_ids.len() > MAX_RECORDS {
        return Err("too-many-run-ids".into());
    }
    let mut unique = HashSet::new();
    for run_id in run_ids {
        Uuid::parse_str(run_id).map_err(|_| "invalid-run-id".to_string())?;
        if !unique.insert(run_id.as_str()) {
            return Err("duplicate-run-id".into());
        }
    }
    Ok(())
}

fn validate_comparison_ids(input: &CompareRuntimeBenchmarksIpc) -> Result<(), String> {
    validate_run_ids(&input.left_run_ids)?;
    validate_run_ids(&input.right_run_ids)?;
    let left_ids: HashSet<&str> = input.left_run_ids.iter().map(String::as_str).collect();
    if input
        .right_run_ids
        .iter()
        .any(|id| left_ids.contains(id.as_str()))
    {
        return Err("compare-overlapping-runs".into());
    }
    Ok(())
}

fn validate_for_compare(
    record: &RuntimeBenchmarkRunV1,
    expected_arm: BenchmarkArm,
) -> Result<(), String> {
    if record.arm != expected_arm {
        return Err("compare-arm-mismatch".into());
    }
    if record.record_state == BenchmarkRecordState::Invalid {
        return Err("run-invalid".into());
    }
    if record.visual_check == BenchmarkVisualCheck::Pending {
        return Err("visual-check-pending".into());
    }
    Ok(())
}

fn identity_matches_record(record: &RuntimeBenchmarkRunV1, identity: &ProcessIdentity) -> bool {
    record
        .process
        .game
        .as_ref()
        .is_some_and(|game| game.pid == identity.pid && game.start_time == identity.start_time)
}

fn invalidate(record: &mut RuntimeBenchmarkRunV1, reason: BenchmarkInvalidReason) {
    record.record_state = BenchmarkRecordState::Invalid;
    record.invalid_reason = Some(reason);
}

fn client_has_active_capture(
    paths: &BenchmarkPaths,
    client_id: &str,
    except_run_id: Option<&str>,
) -> Result<bool, String> {
    if !paths.root.is_dir() {
        return Ok(false);
    }
    for entry in fs::read_dir(&paths.root)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let path = entry.path();
        if let Ok(mut record) = read_record(&path) {
            if record.client_id != client_id {
                continue;
            }
            if except_run_id == Some(record.run_id.as_str()) {
                continue;
            }
            if matches!(
                record.record_state,
                BenchmarkRecordState::Attached | BenchmarkRecordState::Capturing
            ) {
                if !benchmark_process_is_alive(&record) {
                    invalidate(&mut record, BenchmarkInvalidReason::ProcessGone);
                    write_record(paths, &record)?;
                    continue;
                }
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn benchmark_process_is_alive(record: &RuntimeBenchmarkRunV1) -> bool {
    record.process.game.as_ref().is_some_and(|game| {
        verify_process_identity(&ProcessIdentity {
            pid: game.pid,
            start_time: game.start_time,
        })
    })
}

fn write_record(paths: &BenchmarkPaths, record: &RuntimeBenchmarkRunV1) -> Result<(), String> {
    fs::create_dir_all(&paths.root).map_err(|error| error.to_string())?;
    let redacted = redact_benchmark_for_disk(record);
    replace_json(&record_path(paths, &record.run_id)?, &redacted)
}

fn redact_benchmark_for_disk(record: &RuntimeBenchmarkRunV1) -> RuntimeBenchmarkRunV1 {
    let home = std::env::var("HOME").unwrap_or_default();
    let app_data = app_data_dir().to_string_lossy().to_string();
    let secrets: Vec<String> = [home, app_data]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect();
    let json = serde_json::to_value(record).expect("benchmark json");
    let redacted = redact_json_strings(&json, &secrets);
    serde_json::from_value(redacted).expect("benchmark roundtrip")
}

fn redact_json_strings(value: &serde_json::Value, secrets: &[String]) -> serde_json::Value {
    use crate::utils::process::redact_sensitive_values;
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(redact_sensitive_values(text, secrets))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| redact_json_strings(item, secrets))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map {
                out.insert(key.clone(), redact_json_strings(item, secrets));
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

fn read_record(path: &Path) -> Result<RuntimeBenchmarkRunV1, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("benchmark inválido en {}: {error}", path.display()))?;
    if value.get("schemaVersion").and_then(|field| field.as_u64())
        != Some(RUNTIME_BENCHMARK_SCHEMA_VERSION as u64)
    {
        quarantine_corrupt(path)?;
        return Err("unsupported-benchmark-schema".into());
    }
    let record: RuntimeBenchmarkRunV1 = serde_json::from_value(value)
        .map_err(|error| format!("benchmark inválido en {}: {error}", path.display()))?;
    Ok(record)
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
            .unwrap_or("run.json"),
        timestamp
    ));
    fs::rename(path, corrupt).map_err(|error| error.to_string())
}

fn prune(paths: &BenchmarkPaths, keep_id: &str) -> Result<(), String> {
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
            .is_some_and(|name| name.starts_with("run-") && name.ends_with(".json"))
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
        if record.run_id == keep_id {
            continue;
        }
        if let Ok(started) = chrono::DateTime::parse_from_rfc3339(&record.attached_at) {
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
            .attached_at
            .cmp(&right.1.attached_at)
            .then_with(|| left.1.run_id.cmp(&right.1.run_id))
    });
    let excess = records.len() - MAX_RECORDS;
    for (path, record) in records.into_iter().take(excess) {
        if record.run_id == keep_id {
            continue;
        }
        let _ = fs::remove_file(path);
    }
    Ok(())
}

fn summary_from_record(record: &RuntimeBenchmarkRunV1) -> BenchmarkRunSummaryIpc {
    BenchmarkRunSummaryIpc {
        run_id: record.run_id.clone(),
        record_state: record_state_label(record.record_state),
        attached_at: record.attached_at.clone(),
        arm: arm_label(record.arm).to_string(),
        scene_id: record.spec.scene_id.clone(),
        plan_id: record.plan_id.clone(),
        graphics_profile: record.graphics_profile.clone(),
        visual_check: visual_check_label(record.visual_check),
        sample_count: record.samples.as_ref().map(|samples| samples.sample_count),
        invalid_reason: record.invalid_reason.map(invalid_reason_label),
    }
}

fn export_record(record: &RuntimeBenchmarkRunV1) -> RuntimeBenchmarkExportV1 {
    let client_token = sha256_hex(record.client_id.as_bytes())[..16].to_string();
    RuntimeBenchmarkExportV1 {
        run_id: record.run_id.clone(),
        record_state: record_state_label(record.record_state),
        invalid_reason: record.invalid_reason.map(invalid_reason_label),
        attached_at: record.attached_at.clone(),
        capture_started_at: record.capture_started_at.clone(),
        capture_finished_at: record.capture_finished_at.clone(),
        client_token,
        server_token: record.server_token.clone(),
        observation_id: record.observation_id.clone(),
        arm: arm_label(record.arm).to_string(),
        spec: record.spec.clone(),
        plan_id: record.plan_id.clone(),
        runtime_fingerprint: record.runtime_fingerprint.clone(),
        prefix_fingerprint: record.prefix_fingerprint.clone(),
        prefix_token: record.prefix_token.clone(),
        selection_source: record.selection_source.clone(),
        runner_kind: record.runner_kind.clone(),
        graphics_profile: record.graphics_profile.clone(),
        dxvk_provider: record.dxvk_provider.clone(),
        dxvk_component_id: record.dxvk_component_id.clone(),
        overlay_verified: record.overlay_verified,
        subject: record.subject.clone(),
        host_gpu: record.host_gpu.clone(),
        process: record.process.clone(),
        flags: record.flags.clone(),
        capture_adapter: record.capture_adapter,
        samples: record.samples.clone(),
        visual_check: visual_check_label(record.visual_check),
        linked_outcome_kind: record.linked_outcome_kind.clone(),
    }
}

pub(crate) fn build_invalid_at_attach(
    linked_outcome: Option<&str>,
) -> Option<BenchmarkInvalidReason> {
    linked_outcome
        .filter(|kind| linked_outcome_unusable(kind))
        .map(|_| BenchmarkInvalidReason::LinkedOutcomeNotUsable)
}

pub(crate) fn to_process_identity_ipc(identity: ProcessIdentity) -> ProcessIdentityIpc {
    ProcessIdentityIpc {
        pid: identity.pid,
        start_time: identity.start_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static BENCH_TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ro-bench-store-{}-{}",
            std::process::id(),
            BENCH_TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn unsupported_schema_is_quarantined() {
        let root = temp_root();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("run-bad.json");
        fs::write(&path, r#"{"schemaVersion":2}"#).unwrap();
        assert!(read_record(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn run_ids_cannot_escape_the_benchmark_directory() {
        let paths = BenchmarkPaths { root: temp_root() };
        assert_eq!(
            record_path(&paths, "../../outside").unwrap_err(),
            "invalid-run-id"
        );
    }

    #[test]
    fn comparison_ids_reject_duplicates_and_overlap() {
        let id = Uuid::new_v4().to_string();
        assert_eq!(
            validate_run_ids(&[id.clone(), id.clone()]).unwrap_err(),
            "duplicate-run-id"
        );
        let input = CompareRuntimeBenchmarksIpc {
            left_run_ids: vec![id.clone()],
            right_run_ids: vec![id],
        };
        assert_eq!(
            validate_comparison_ids(&input).unwrap_err(),
            "compare-overlapping-runs"
        );
    }
}
