use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub executable_path: String,
    pub patcher_path: Option<String>,
    pub wine_prefix: Option<String>,
    pub runner: Option<String>,
}
