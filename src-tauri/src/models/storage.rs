use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageNoticeSource {
    Servers,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageNoticeKind {
    Migrated,
    Recovered,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageNotice {
    pub source: StorageNoticeSource,
    pub kind: StorageNoticeKind,
    pub message: String,
}
