use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeObservationSummaryIpc {
    pub observation_id: String,
    pub record_state: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub server_token: String,
    pub plan_id: String,
    pub outcome_kind: Option<String>,
    pub visual_check: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeObservationExportResultIpc {
    pub exported_count: usize,
}
