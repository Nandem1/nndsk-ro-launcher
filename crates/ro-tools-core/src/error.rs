use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolsError {
    #[error("memory read failed at {address:#x}: {message}")]
    MemoryRead { address: u32, message: String },
    #[error("input failed for key '{key}': {message}")]
    Input { key: String, message: String },
    #[error("{0}")]
    Other(String),
}
