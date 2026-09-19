use serde::{Deserialize, Serialize};

use super::server::ServerConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResolutionIpc {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRuntimeBenchmarkRunIpc {
    pub client_id: String,
    pub server: ServerConfig,
    pub runner: Option<String>,
    pub arm: String,
    pub scene_id: String,
    pub load_descriptor: String,
    pub resolution: BenchmarkResolutionIpc,
    pub warmup_seconds: u32,
    pub capture_seconds: u32,
    #[serde(default = "default_thermal_declared")]
    pub thermal_declared: String,
}

fn default_thermal_declared() -> String {
    "unmeasured".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkRunSummaryIpc {
    pub run_id: String,
    pub record_state: String,
    pub attached_at: String,
    pub arm: String,
    pub scene_id: String,
    pub plan_id: String,
    pub graphics_profile: String,
    pub visual_check: String,
    pub sample_count: Option<u32>,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRuntimeBenchmarksIpc {
    pub left_run_ids: Vec<String>,
    pub right_run_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ComparabilityIpc {
    Comparable,
    Incomparable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrametimeMetricsIpc {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub one_percent_low_fps: f64,
    pub point_one_percent_low_fps: f64,
    pub sample_count: u32,
    pub run_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArmMetricsIpc {
    pub arm: String,
    pub attempts: u32,
    pub capture_successes: u32,
    pub plan_ids: Vec<String>,
    pub frametime: Option<FrametimeMetricsIpc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsDeltaIpc {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub one_percent_low_fps: f64,
    pub point_one_percent_low_fps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonGatesIpc {
    pub visual_correct_left: bool,
    pub visual_correct_right: bool,
    pub startup_clean_left: bool,
    pub startup_clean_right: bool,
    pub frametime_available: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkComparisonIpc {
    pub schema_version: u32,
    pub comparability: ComparabilityIpc,
    pub left: ArmMetricsIpc,
    pub right: ArmMetricsIpc,
    pub deltas: Option<MetricsDeltaIpc>,
    pub gates: ComparisonGatesIpc,
    pub caveats: Vec<String>,
    pub order_bias_uncontrolled: bool,
    pub usable_for_autotune: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkExportResultIpc {
    pub exported_count: usize,
}
