use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::benchmark::{
    ArmMetricsIpc, BenchmarkComparisonIpc, ComparabilityIpc, ComparisonGatesIpc,
    FrametimeMetricsIpc, MetricsDeltaIpc,
};

use super::compatibility::SubjectObservation;
use super::encode::sha256_hex;
use super::fingerprint::RuntimeFingerprint;
use super::host_gpu::HostGpuObservationIpc;
use super::observation::{FingerprintEnvelopeIpc, ObservationProcessIpc, ObservationSubjectIpc};

pub(crate) const RUNTIME_BENCHMARK_SCHEMA_VERSION: u32 = 1;
pub(crate) const BENCHMARK_PROTOCOL_REVISION: u64 = 1;
const MAX_CSV_BYTES: usize = 32 * 1024 * 1024;
const MAX_CSV_SAMPLES: usize = 2_000_000;

pub(crate) fn runtime_benchmark_enabled() -> bool {
    runtime_benchmark_enabled_from(
        std::env::var("RO_LAUNCHER_RUNTIME_BENCHMARK")
            .ok()
            .as_deref(),
    )
}

pub(crate) fn runtime_benchmark_enabled_from(value: Option<&str>) -> bool {
    value != Some("0")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BenchmarkRecordState {
    Attached,
    Capturing,
    CaptureFinished,
    SamplesImported,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum BenchmarkArm {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CaptureAdapterKind {
    ImportedCsvV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BenchmarkVisualCheck {
    Pending,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThermalDeclared {
    Unmeasured,
    Cool,
    Throttled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BenchmarkInvalidReason {
    ProcessGone,
    PlanChanged,
    WarmupTooShort,
    DurationMismatch,
    ClockJumpOrPause,
    InsufficientSamples,
    LinkedOutcomeNotUsable,
    CorruptRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BenchmarkResolutionV1 {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BenchmarkSpecV1 {
    pub protocol_revision: u64,
    pub scene_id: String,
    pub load_descriptor: String,
    pub resolution: BenchmarkResolutionV1,
    pub warmup_seconds: u32,
    pub capture_seconds: u32,
    pub thermal_declared: ThermalDeclared,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BenchmarkFlagsIpc {
    pub graphics_plan: bool,
    pub supervisor: bool,
    pub observe: bool,
    pub benchmark: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BenchmarkSamplesV1 {
    pub adapter: CaptureAdapterKind,
    pub source_sha256: String,
    pub sample_count: u32,
    pub frametime_ms: Vec<f64>,
    pub monotonic_ns: Option<Vec<u64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeBenchmarkRunV1 {
    pub schema_version: u32,
    pub run_id: String,
    pub record_state: BenchmarkRecordState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<BenchmarkInvalidReason>,
    pub attached_at: String,
    pub capture_started_at: Option<String>,
    pub capture_finished_at: Option<String>,
    pub client_id: String,
    pub server_local_id: String,
    pub server_token: String,
    pub observation_id: Option<String>,
    pub arm: BenchmarkArm,
    pub spec: BenchmarkSpecV1,
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
    pub subject: ObservationSubjectIpc,
    pub host_gpu: HostGpuObservationIpc,
    pub process: ObservationProcessIpc,
    pub flags: BenchmarkFlagsIpc,
    pub capture_adapter: CaptureAdapterKind,
    pub samples: Option<BenchmarkSamplesV1>,
    pub visual_check: BenchmarkVisualCheck,
    pub linked_outcome_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedCsvSamples {
    pub frametime_ms: Vec<f64>,
    pub monotonic_ns: Option<Vec<u64>>,
    pub source_sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrametimeMetrics {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub one_percent_low_fps: f64,
    pub point_one_percent_low_fps: f64,
    pub sample_count: u32,
    pub run_count: u32,
}

pub(crate) fn validate_scene_token(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    let mut chars = value.chars();
    let first = chars.next().expect("non-empty");
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

pub(crate) fn parse_arm(value: &str) -> Option<BenchmarkArm> {
    match value {
        "a" => Some(BenchmarkArm::A),
        "b" => Some(BenchmarkArm::B),
        _ => None,
    }
}

pub(crate) fn parse_thermal(value: &str) -> Option<ThermalDeclared> {
    match value {
        "unmeasured" => Some(ThermalDeclared::Unmeasured),
        "cool" => Some(ThermalDeclared::Cool),
        "throttled" => Some(ThermalDeclared::Throttled),
        _ => None,
    }
}

pub(crate) fn parse_visual_check(value: &str) -> Option<BenchmarkVisualCheck> {
    match value {
        "pending" => Some(BenchmarkVisualCheck::Pending),
        "passed" => Some(BenchmarkVisualCheck::Passed),
        "failed" => Some(BenchmarkVisualCheck::Failed),
        "skipped" => Some(BenchmarkVisualCheck::Skipped),
        _ => None,
    }
}

pub(crate) fn validate_spec(
    scene_id: &str,
    load_descriptor: &str,
    width: u32,
    height: u32,
    warmup_seconds: u32,
    capture_seconds: u32,
) -> Result<(), String> {
    if !validate_scene_token(scene_id) || !validate_scene_token(load_descriptor) {
        return Err("invalid-scene-token".into());
    }
    if warmup_seconds > 600 {
        return Err("invalid-warmup-seconds".into());
    }
    if !(10..=3600).contains(&capture_seconds) {
        return Err("invalid-capture-seconds".into());
    }
    if !(1..=7680).contains(&width) || !(1..=4320).contains(&height) {
        return Err("invalid-resolution".into());
    }
    Ok(())
}

pub(crate) fn process_age_seconds(
    start_time_ticks: u64,
    uptime_secs: f64,
    clk_tck: i64,
) -> Option<f64> {
    if clk_tck <= 0 {
        return None;
    }
    Some(uptime_secs - (start_time_ticks as f64 / clk_tck as f64))
}

pub(crate) fn host_uptime_seconds() -> Option<f64> {
    let content = std::fs::read_to_string("/proc/uptime").ok()?;
    let field = content.split_whitespace().next()?;
    field.parse().ok()
}

pub(crate) fn host_clk_tck() -> i64 {
    unsafe { libc::sysconf(libc::_SC_CLK_TCK) }
}

pub(crate) fn percentile_nearest_rank(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    assert!(n >= 1);
    let rank = (p * n as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(n - 1);
    sorted[index]
}

pub(crate) fn percent_low_fps(sorted: &[f64], fraction: f64) -> f64 {
    let n = sorted.len();
    let count = ((n as f64) * fraction).ceil() as usize;
    let count = count.max(1).min(n);
    let mean = sorted[n - count..].iter().sum::<f64>() / count as f64;
    1000.0 / mean
}

pub(crate) fn compute_frametime_metrics(
    frametime_ms: &[f64],
    run_count: u32,
) -> Option<FrametimeMetrics> {
    if frametime_ms.is_empty() {
        return None;
    }
    let mut sorted = frametime_ms.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap());
    Some(FrametimeMetrics {
        p50_ms: percentile_nearest_rank(&sorted, 0.50),
        p95_ms: percentile_nearest_rank(&sorted, 0.95),
        p99_ms: percentile_nearest_rank(&sorted, 0.99),
        one_percent_low_fps: percent_low_fps(&sorted, 0.01),
        point_one_percent_low_fps: percent_low_fps(&sorted, 0.001),
        sample_count: sorted.len() as u32,
        run_count,
    })
}

pub(crate) fn parse_imported_csv_v1(path: &Path) -> Result<ParsedCsvSamples, String> {
    if path.is_relative() {
        return Err("csv-path-rejected".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "csv-read-failed".to_string())?;
    if bytes.len() > MAX_CSV_BYTES {
        return Err("csv-too-large".into());
    }
    let source_sha256 = sha256_hex(&bytes);
    let mut content = String::from_utf8(bytes).map_err(|_| "csv-invalid-encoding".to_string())?;
    if content.starts_with('\u{feff}') {
        content = content.trim_start_matches('\u{feff}').to_string();
    }
    let mut frametime_ms = Vec::new();
    let mut monotonic_ns = None;
    let mut header_seen = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !header_seen {
            header_seen = true;
            match line {
                "frametimeMs" => {}
                "frametimeMs,monotonicNs" => monotonic_ns = Some(Vec::new()),
                _ => return Err("csv-invalid-header".into()),
            }
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        let expected_columns = if monotonic_ns.is_some() { 2 } else { 1 };
        if parts.len() != expected_columns {
            return Err("csv-invalid-row".into());
        }
        let value = parts[0]
            .parse::<f64>()
            .map_err(|_| "csv-invalid-row".to_string())?;
        if !value.is_finite() || value <= 0.0 || value >= 60000.0 {
            return Err("csv-invalid-row".into());
        }
        frametime_ms.push(value);
        if let Some(ref mut mono) = monotonic_ns {
            let ns = parts[1]
                .parse::<u64>()
                .map_err(|_| "csv-invalid-row".to_string())?;
            if let Some(prev) = mono.last() {
                if ns <= *prev {
                    return Err("csv-clock-not-monotonic".into());
                }
            }
            mono.push(ns);
        }
        if frametime_ms.len() > MAX_CSV_SAMPLES {
            return Err("csv-too-large".into());
        }
    }
    if !header_seen || frametime_ms.is_empty() {
        return Err("csv-empty".into());
    }
    Ok(ParsedCsvSamples {
        frametime_ms,
        monotonic_ns,
        source_sha256,
    })
}

pub(crate) fn min_samples_for_capture(capture_seconds: u32) -> usize {
    usize::max(30, capture_seconds as usize)
}

pub(crate) fn duration_within_tolerance(
    started: &str,
    finished: &str,
    expected_seconds: u32,
) -> Result<bool, String> {
    use chrono::DateTime;
    let start =
        DateTime::parse_from_rfc3339(started).map_err(|_| "invalid-timestamp".to_string())?;
    let end =
        DateTime::parse_from_rfc3339(finished).map_err(|_| "invalid-timestamp".to_string())?;
    let actual = (end - start).num_milliseconds() as f64 / 1000.0;
    let expected = expected_seconds as f64;
    Ok(actual >= 0.9 * expected && actual <= 1.1 * expected)
}

fn subject_key(subject: &ObservationSubjectIpc) -> Option<String> {
    match subject {
        ObservationSubjectIpc::Absent => Some("absent".to_string()),
        ObservationSubjectIpc::Unreadable => Some("unreadable".to_string()),
        ObservationSubjectIpc::Gepard { sha256, .. } => Some(format!("gepard:{sha256}")),
    }
}

fn gpu_cards_key(host: &HostGpuObservationIpc) -> String {
    let mut cards = host.cards.clone();
    cards.sort_by(|left, right| left.sysfs_card.cmp(&right.sysfs_card));
    cards
        .iter()
        .map(|card| {
            format!(
                "{}:{}:{}:{}",
                card.sysfs_card,
                card.vendor_id.as_deref().unwrap_or(""),
                card.device_id.as_deref().unwrap_or(""),
                card.driver.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

pub(crate) fn comparability_key(run: &RuntimeBenchmarkRunV1) -> String {
    format!(
        "p{}|{}|{}|{}x{}|{}|{}|{}|srv{}|sub{}|rk{}|pfx{}|gpu{}|{}|gp{}|sup{}|adp{}|th{}",
        run.spec.protocol_revision,
        run.spec.scene_id,
        run.spec.load_descriptor,
        run.spec.resolution.width,
        run.spec.resolution.height,
        run.spec.resolution.fullscreen,
        run.spec.warmup_seconds,
        run.spec.capture_seconds,
        run.server_token,
        subject_key(&run.subject).unwrap_or_default(),
        run.runner_kind,
        run.prefix_fingerprint.digest,
        gpu_cards_key(&run.host_gpu),
        run.host_gpu.completeness,
        run.flags.graphics_plan,
        run.flags.supervisor,
        serde_json::to_string(&run.capture_adapter).unwrap_or_default(),
        serde_json::to_string(&run.spec.thermal_declared).unwrap_or_default(),
    )
}

pub(crate) fn arm_internal_homogeneous(runs: &[&RuntimeBenchmarkRunV1]) -> bool {
    if runs.is_empty() {
        return true;
    }
    let key = comparability_key(runs[0]);
    runs.iter().all(|run| comparability_key(run) == key)
}

pub(crate) fn assess_pair_comparability(
    left: &RuntimeBenchmarkRunV1,
    right: &RuntimeBenchmarkRunV1,
) -> ComparabilityIpc {
    if left.host_gpu.completeness != "known" || right.host_gpu.completeness != "known" {
        return ComparabilityIpc::Incomparable {
            reason: "host-gpu-incomplete".to_string(),
        };
    }
    if comparability_key(left) != comparability_key(right) {
        if left.spec.scene_id != right.spec.scene_id {
            return ComparabilityIpc::Incomparable {
                reason: "scene-mismatch".to_string(),
            };
        }
        if left.prefix_fingerprint.digest != right.prefix_fingerprint.digest {
            return ComparabilityIpc::Incomparable {
                reason: "prefix-fingerprint-mismatch".to_string(),
            };
        }
        if gpu_cards_key(&left.host_gpu) != gpu_cards_key(&right.host_gpu) {
            return ComparabilityIpc::Incomparable {
                reason: "host-gpu-mismatch".to_string(),
            };
        }
        if subject_key(&left.subject) != subject_key(&right.subject) {
            return ComparabilityIpc::Incomparable {
                reason: "subject-mismatch".to_string(),
            };
        }
        if left.spec.thermal_declared != right.spec.thermal_declared {
            return ComparabilityIpc::Incomparable {
                reason: "thermal-mismatch".to_string(),
            };
        }
        if left.flags.graphics_plan != right.flags.graphics_plan {
            return ComparabilityIpc::Incomparable {
                reason: "graphics-plan-flag-mismatch".to_string(),
            };
        }
        return ComparabilityIpc::Incomparable {
            reason: "protocol-mismatch".to_string(),
        };
    }
    ComparabilityIpc::Comparable
}

pub(crate) fn is_frametime_eligible(run: &RuntimeBenchmarkRunV1) -> bool {
    run.record_state == BenchmarkRecordState::SamplesImported
        && run.invalid_reason.is_none()
        && run.visual_check == BenchmarkVisualCheck::Passed
        && run.samples.is_some()
}

pub(crate) fn collect_frametimes(runs: &[&RuntimeBenchmarkRunV1]) -> Vec<f64> {
    let mut out = Vec::new();
    for run in runs {
        if is_frametime_eligible(run) {
            if let Some(samples) = &run.samples {
                out.extend(samples.frametime_ms.iter().copied());
            }
        }
    }
    out
}

const BAD_OUTCOMES: &[&str] = &["startupFailed", "startupTimeout", "crashConfirmed"];

pub(crate) fn visual_correct_for_arm(runs: &[&RuntimeBenchmarkRunV1]) -> bool {
    runs.iter()
        .all(|run| run.visual_check != BenchmarkVisualCheck::Failed)
        && runs
            .iter()
            .any(|run| run.visual_check == BenchmarkVisualCheck::Passed)
}

pub(crate) fn startup_clean_for_arm(runs: &[&RuntimeBenchmarkRunV1]) -> bool {
    let attempts = runs.len();
    let successes = runs
        .iter()
        .filter(|run| {
            run.record_state == BenchmarkRecordState::SamplesImported
                && run.invalid_reason.is_none()
        })
        .count();
    if successes != attempts {
        return false;
    }
    !runs.iter().any(|run| {
        run.linked_outcome_kind
            .as_deref()
            .is_some_and(|kind| BAD_OUTCOMES.contains(&kind))
    })
}

pub(crate) fn order_bias_uncontrolled(
    left: &[&RuntimeBenchmarkRunV1],
    right: &[&RuntimeBenchmarkRunV1],
) -> bool {
    let mut combined: Vec<(&RuntimeBenchmarkRunV1, bool)> = left
        .iter()
        .map(|run| (*run, true))
        .chain(right.iter().map(|run| (*run, false)))
        .collect();
    combined.sort_by(|left, right| left.0.attached_at.cmp(&right.0.attached_at));
    let mut saw_left = false;
    let mut saw_right = false;
    let mut left_before_right_done = false;
    let mut right_before_left_done = false;
    for (_, is_left) in combined {
        if is_left {
            saw_left = true;
            if saw_right {
                right_before_left_done = true;
            }
        } else {
            saw_right = true;
            if saw_left {
                left_before_right_done = true;
            }
        }
    }
    (saw_left && saw_right)
        && ((left_before_right_done && !right_before_left_done)
            || (right_before_left_done && !left_before_right_done))
}

pub(crate) fn compare_benchmark_runs(
    left_runs: Vec<RuntimeBenchmarkRunV1>,
    right_runs: Vec<RuntimeBenchmarkRunV1>,
) -> BenchmarkComparisonIpc {
    let left_refs: Vec<&RuntimeBenchmarkRunV1> = left_runs.iter().collect();
    let right_refs: Vec<&RuntimeBenchmarkRunV1> = right_runs.iter().collect();
    let mut caveats = Vec::new();

    let comparability =
        if !arm_internal_homogeneous(&left_refs) || !arm_internal_homogeneous(&right_refs) {
            ComparabilityIpc::Incomparable {
                reason: "arm-internal-mismatch".to_string(),
            }
        } else if left_refs.is_empty() || right_refs.is_empty() {
            ComparabilityIpc::Incomparable {
                reason: "compare-empty-arm".to_string(),
            }
        } else {
            assess_pair_comparability(left_refs[0], right_refs[0])
        };

    if matches!(comparability, ComparabilityIpc::Comparable)
        && left_refs[0].spec.thermal_declared == ThermalDeclared::Unmeasured
        && right_refs[0].spec.thermal_declared == ThermalDeclared::Unmeasured
    {
        caveats.push("thermal-unmeasured".to_string());
    }

    let plan_ids: std::collections::BTreeSet<String> = left_runs
        .iter()
        .chain(right_runs.iter())
        .map(|run| run.plan_id.clone())
        .collect();
    if plan_ids.len() == 1 {
        caveats.push("same-runtime-plan".to_string());
    }

    let left_ft = collect_frametimes(&left_refs);
    let right_ft = collect_frametimes(&right_refs);
    let left_metrics = compute_frametime_metrics(&left_ft, left_refs.len() as u32);
    let right_metrics = compute_frametime_metrics(&right_ft, right_refs.len() as u32);

    if left_metrics.as_ref().is_some_and(|m| m.sample_count == 10) {
        caveats.push("low-sample-1pct".to_string());
    }

    let visual_correct_left = visual_correct_for_arm(&left_refs);
    let visual_correct_right = visual_correct_for_arm(&right_refs);
    let startup_clean_left = startup_clean_for_arm(&left_refs);
    let startup_clean_right = startup_clean_for_arm(&right_refs);
    let frametime_available = left_metrics.is_some() && right_metrics.is_some();
    let comparable = matches!(comparability, ComparabilityIpc::Comparable);
    let passed = comparable
        && visual_correct_left
        && visual_correct_right
        && startup_clean_left
        && startup_clean_right
        && frametime_available;

    let deltas = if comparable && frametime_available {
        let left = left_metrics.as_ref().expect("metrics");
        let right = right_metrics.as_ref().expect("metrics");
        Some(MetricsDeltaIpc {
            p50_ms: right.p50_ms - left.p50_ms,
            p95_ms: right.p95_ms - left.p95_ms,
            p99_ms: right.p99_ms - left.p99_ms,
            one_percent_low_fps: right.one_percent_low_fps - left.one_percent_low_fps,
            point_one_percent_low_fps: right.point_one_percent_low_fps
                - left.point_one_percent_low_fps,
        })
    } else {
        None
    };

    let order_bias = order_bias_uncontrolled(&left_refs, &right_refs);
    let usable_for_autotune = comparable
        && passed
        && !order_bias
        && left_metrics.as_ref().is_some_and(|m| m.run_count >= 2)
        && right_metrics.as_ref().is_some_and(|m| m.run_count >= 2);

    BenchmarkComparisonIpc {
        schema_version: 1,
        comparability,
        left: arm_metrics_ipc("a", &left_refs, left_metrics),
        right: arm_metrics_ipc("b", &right_refs, right_metrics),
        deltas,
        gates: ComparisonGatesIpc {
            visual_correct_left,
            visual_correct_right,
            startup_clean_left,
            startup_clean_right,
            frametime_available,
            passed,
        },
        caveats,
        order_bias_uncontrolled: order_bias,
        usable_for_autotune,
    }
}

fn arm_metrics_ipc(
    arm: &str,
    runs: &[&RuntimeBenchmarkRunV1],
    metrics: Option<FrametimeMetrics>,
) -> ArmMetricsIpc {
    let attempts = runs.len() as u32;
    let capture_successes = runs
        .iter()
        .filter(|run| {
            run.record_state == BenchmarkRecordState::SamplesImported
                && run.invalid_reason.is_none()
        })
        .count() as u32;
    let plan_ids = runs.iter().map(|run| run.plan_id.clone()).collect();
    ArmMetricsIpc {
        arm: arm.to_string(),
        attempts,
        capture_successes,
        plan_ids,
        frametime: metrics.map(|m| FrametimeMetricsIpc {
            p50_ms: m.p50_ms,
            p95_ms: m.p95_ms,
            p99_ms: m.p99_ms,
            one_percent_low_fps: m.one_percent_low_fps,
            point_one_percent_low_fps: m.point_one_percent_low_fps,
            sample_count: m.sample_count,
            run_count: m.run_count,
        }),
    }
}

pub(crate) fn arm_label(arm: BenchmarkArm) -> &'static str {
    match arm {
        BenchmarkArm::A => "a",
        BenchmarkArm::B => "b",
    }
}

pub(crate) fn record_state_label(state: BenchmarkRecordState) -> String {
    match state {
        BenchmarkRecordState::Attached => "attached".to_string(),
        BenchmarkRecordState::Capturing => "capturing".to_string(),
        BenchmarkRecordState::CaptureFinished => "capture-finished".to_string(),
        BenchmarkRecordState::SamplesImported => "samples-imported".to_string(),
        BenchmarkRecordState::Invalid => "invalid".to_string(),
    }
}

pub(crate) fn invalid_reason_label(reason: BenchmarkInvalidReason) -> String {
    match reason {
        BenchmarkInvalidReason::ProcessGone => "process-gone".to_string(),
        BenchmarkInvalidReason::PlanChanged => "plan-changed".to_string(),
        BenchmarkInvalidReason::WarmupTooShort => "warmup-too-short".to_string(),
        BenchmarkInvalidReason::DurationMismatch => "duration-mismatch".to_string(),
        BenchmarkInvalidReason::ClockJumpOrPause => "clock-jump-or-pause".to_string(),
        BenchmarkInvalidReason::InsufficientSamples => "insufficient-samples".to_string(),
        BenchmarkInvalidReason::LinkedOutcomeNotUsable => "linked-outcome-not-usable".to_string(),
        BenchmarkInvalidReason::CorruptRecord => "corrupt-record".to_string(),
    }
}

pub(crate) fn visual_check_label(check: BenchmarkVisualCheck) -> String {
    match check {
        BenchmarkVisualCheck::Pending => "pending".to_string(),
        BenchmarkVisualCheck::Passed => "passed".to_string(),
        BenchmarkVisualCheck::Failed => "failed".to_string(),
        BenchmarkVisualCheck::Skipped => "skipped".to_string(),
    }
}

pub(crate) fn linked_outcome_unusable(kind: &str) -> bool {
    BAD_OUTCOMES.contains(&kind)
}

pub(crate) fn map_subject_observation(subject: SubjectObservation) -> ObservationSubjectIpc {
    match subject {
        SubjectObservation::Absent => ObservationSubjectIpc::Absent,
        SubjectObservation::Unreadable => ObservationSubjectIpc::Unreadable,
        SubjectObservation::Hash { sha256_lowercase } => ObservationSubjectIpc::Gepard {
            sha256: sha256_lowercase,
            file_name: "gepard.dll".to_string(),
        },
    }
}

pub(crate) fn runtime_fingerprint_envelope(fp: &RuntimeFingerprint) -> FingerprintEnvelopeIpc {
    FingerprintEnvelopeIpc {
        schema_version: fp.digest.schema_version,
        algorithm: fp.digest.algorithm.clone(),
        digest: fp.digest.hex_digest(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::models::benchmark::{BenchmarkComparisonIpc, ComparabilityIpc};

    #[test]
    fn benchmark_flag_semantics() {
        assert!(runtime_benchmark_enabled_from(None));
        assert!(runtime_benchmark_enabled_from(Some("1")));
        assert!(runtime_benchmark_enabled_from(Some("false")));
        assert!(!runtime_benchmark_enabled_from(Some("0")));
    }

    #[test]
    fn percentile_nearest_rank_literals() {
        assert_eq!(percentile_nearest_rank(&[1.0, 2.0, 3.0], 0.5), 2.0);
        let sorted: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile_nearest_rank(&sorted, 0.50), 50.0);
        assert_eq!(percentile_nearest_rank(&sorted, 0.95), 95.0);
        assert_eq!(percentile_nearest_rank(&sorted, 0.99), 99.0);
        assert_eq!(percent_low_fps(&sorted, 0.01), 10.0);
        assert_eq!(percent_low_fps(&sorted, 0.001), 10.0);
    }

    #[test]
    fn process_age_seconds_fixture() {
        assert_eq!(process_age_seconds(1000, 10.5, 100), Some(0.5));
    }

    fn sample_run(
        arm: BenchmarkArm,
        scene: &str,
        visual: BenchmarkVisualCheck,
    ) -> RuntimeBenchmarkRunV1 {
        RuntimeBenchmarkRunV1 {
            schema_version: 1,
            run_id: "r".to_string(),
            record_state: BenchmarkRecordState::SamplesImported,
            invalid_reason: None,
            attached_at: "2026-01-01T00:00:00Z".to_string(),
            capture_started_at: None,
            capture_finished_at: None,
            client_id: "c".to_string(),
            server_local_id: "s".to_string(),
            server_token: "tok".to_string(),
            observation_id: None,
            arm,
            spec: BenchmarkSpecV1 {
                protocol_revision: 1,
                scene_id: scene.to_string(),
                load_descriptor: "load".to_string(),
                resolution: BenchmarkResolutionV1 {
                    width: 1920,
                    height: 1080,
                    fullscreen: false,
                },
                warmup_seconds: 0,
                capture_seconds: 30,
                thermal_declared: ThermalDeclared::Unmeasured,
            },
            plan_id: "plan".to_string(),
            runtime_fingerprint: FingerprintEnvelopeIpc {
                schema_version: 1,
                algorithm: "sha256".to_string(),
                digest: "aa".repeat(32),
            },
            prefix_fingerprint: FingerprintEnvelopeIpc {
                schema_version: 1,
                algorithm: "sha256".to_string(),
                digest: "bb".repeat(32),
            },
            prefix_token: "pt".to_string(),
            selection_source: "productDefault".to_string(),
            runner_kind: "wine".to_string(),
            graphics_profile: "dxvk".to_string(),
            dxvk_provider: "managedPrefix".to_string(),
            dxvk_component_id: "dxvk-2.6.2".to_string(),
            overlay_verified: false,
            subject: ObservationSubjectIpc::Absent,
            host_gpu: HostGpuObservationIpc {
                completeness: "known".to_string(),
                cards: vec![super::super::host_gpu::HostGpuCardIpc {
                    sysfs_card: "card0".to_string(),
                    vendor_id: Some("10de".to_string()),
                    device_id: Some("2484".to_string()),
                    driver: Some("nvidia".to_string()),
                }],
                vulkan_api: None,
            },
            process: ObservationProcessIpc {
                game: None,
                controller: None,
                final_game: None,
                identity_stale: false,
                handoff_count: 0,
            },
            flags: BenchmarkFlagsIpc {
                graphics_plan: true,
                supervisor: true,
                observe: true,
                benchmark: true,
            },
            capture_adapter: CaptureAdapterKind::ImportedCsvV1,
            samples: Some(BenchmarkSamplesV1 {
                adapter: CaptureAdapterKind::ImportedCsvV1,
                source_sha256: "x".repeat(64),
                sample_count: 3,
                frametime_ms: vec![10.0, 20.0, 30.0],
                monotonic_ns: None,
            }),
            visual_check: visual,
            linked_outcome_kind: None,
        }
    }

    #[test]
    fn scene_mismatch_is_incomparable() {
        let left = sample_run(BenchmarkArm::A, "scene-a", BenchmarkVisualCheck::Passed);
        let right = sample_run(BenchmarkArm::B, "scene-b", BenchmarkVisualCheck::Passed);
        let cmp = compare_benchmark_runs(vec![left], vec![right]);
        assert!(matches!(
            cmp.comparability,
            ComparabilityIpc::Incomparable { reason } if reason == "scene-mismatch"
        ));
    }

    #[test]
    fn visual_failed_blocks_gates_even_with_better_p50() {
        let left = sample_run(BenchmarkArm::A, "scene", BenchmarkVisualCheck::Failed);
        let mut right = sample_run(BenchmarkArm::B, "scene", BenchmarkVisualCheck::Passed);
        right.samples = Some(BenchmarkSamplesV1 {
            adapter: CaptureAdapterKind::ImportedCsvV1,
            source_sha256: "y".repeat(64),
            sample_count: 3,
            frametime_ms: vec![100.0, 100.0, 100.0],
            monotonic_ns: None,
        });
        let cmp = compare_benchmark_runs(vec![left], vec![right]);
        assert!(!cmp.gates.passed);
        assert!(!cmp.gates.visual_correct_left);
    }

    #[test]
    fn golden_metrics_match_fixture_and_hand_computation() {
        let fixture = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-benchmark-metrics-v1.json"
        ))
        .expect("fixture");
        let json: serde_json::Value = serde_json::from_str(&fixture).expect("json");
        let dataset = &json["dataset100"];
        let samples: Vec<f64> = dataset["frametimeMs"]
            .as_array()
            .expect("array")
            .iter()
            .map(|value| value.as_f64().expect("number"))
            .collect();
        let metrics = compute_frametime_metrics(&samples, 1).expect("metrics");
        assert_eq!(metrics.p50_ms, dataset["p50Ms"].as_f64().expect("p50"));
        assert_eq!(metrics.p95_ms, dataset["p95Ms"].as_f64().expect("p95"));
        assert_eq!(metrics.p99_ms, dataset["p99Ms"].as_f64().expect("p99"));
        assert_eq!(
            metrics.one_percent_low_fps,
            dataset["onePercentLowFps"].as_f64().expect("1%")
        );
        let sorted: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(metrics.p50_ms, percentile_nearest_rank(&sorted, 0.50));
    }

    #[test]
    fn imported_csv_v1_fixture_parses() {
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-benchmark-csv-v1.csv"
        ));
        let parsed = parse_imported_csv_v1(path).expect("csv");
        assert_eq!(parsed.frametime_ms.len(), 100);
        assert_eq!(parsed.frametime_ms[0], 1.0);
        assert_eq!(parsed.frametime_ms[99], 100.0);
    }

    #[test]
    fn imported_csv_rejects_extra_or_missing_columns() {
        let one_column = std::env::temp_dir().join(format!(
            "ro-benchmark-csv-one-column-{}.csv",
            std::process::id()
        ));
        std::fs::write(&one_column, "frametimeMs\n16.6,unexpected\n").unwrap();
        assert_eq!(
            parse_imported_csv_v1(&one_column).unwrap_err(),
            "csv-invalid-row"
        );

        let two_columns = std::env::temp_dir().join(format!(
            "ro-benchmark-csv-two-columns-{}.csv",
            std::process::id()
        ));
        std::fs::write(&two_columns, "frametimeMs,monotonicNs\n16.6\n").unwrap();
        assert_eq!(
            parse_imported_csv_v1(&two_columns).unwrap_err(),
            "csv-invalid-row"
        );
        let _ = std::fs::remove_file(one_column);
        let _ = std::fs::remove_file(two_columns);
    }

    #[test]
    #[ignore = "genera contract-fixtures; ejecutar manualmente"]
    fn write_golden_benchmark_fixtures() {
        let left = sample_run(BenchmarkArm::A, "payon-town", BenchmarkVisualCheck::Passed);
        let right = sample_run(BenchmarkArm::B, "payon-town", BenchmarkVisualCheck::Passed);
        let run_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-benchmark-run-v1.json"
        ));
        std::fs::write(
            run_path,
            serde_json::to_string_pretty(&left).expect("serialize"),
        )
        .expect("write run");
        let cmp = compare_benchmark_runs(vec![left], vec![right]);
        let cmp_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-benchmark-comparison-v1.json"
        ));
        std::fs::write(
            cmp_path,
            serde_json::to_string_pretty(&cmp).expect("serialize"),
        )
        .expect("write comparison");
    }

    #[test]
    fn golden_run_fixture_deserializes() {
        let fixture = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-benchmark-run-v1.json"
        ))
        .expect("fixture");
        let run: RuntimeBenchmarkRunV1 = serde_json::from_str(&fixture).expect("json");
        assert_eq!(run.schema_version, 1);
        assert_eq!(run.record_state, BenchmarkRecordState::SamplesImported);
    }

    #[test]
    fn golden_comparison_fixture_deserializes() {
        let fixture = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../contract-fixtures/runtime-benchmark-comparison-v1.json"
        ))
        .expect("fixture");
        let cmp: BenchmarkComparisonIpc = serde_json::from_str(&fixture).expect("json");
        assert_eq!(cmp.schema_version, 1);
        assert!(matches!(cmp.comparability, ComparabilityIpc::Comparable));
    }

    #[test]
    fn order_bias_aa_bb() {
        let mut left = sample_run(BenchmarkArm::A, "scene", BenchmarkVisualCheck::Passed);
        left.attached_at = "2026-01-01T00:00:00Z".to_string();
        let mut right = sample_run(BenchmarkArm::B, "scene", BenchmarkVisualCheck::Passed);
        right.attached_at = "2026-01-02T00:00:00Z".to_string();
        let cmp = compare_benchmark_runs(vec![left], vec![right]);
        assert!(cmp.order_bias_uncontrolled);
    }
}
