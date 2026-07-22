use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GameClientStatus {
    Launching,
    Running,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameClientSnapshot {
    pub client_id: String,
    pub server_id: String,
    pub server_name: String,
    pub status: GameClientStatus,
    pub pid: Option<u32>,
}
