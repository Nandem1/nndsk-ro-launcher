use crate::error::ToolsError;

pub trait MemoryReader: Send + Sync {
    fn read_u32(&self, address: u32) -> Result<u32, ToolsError>;

    /// Null-terminated string (4RTools reads up to 40 bytes).
    fn read_string(&self, address: u32, max_len: usize) -> Result<String, ToolsError>;
}

pub trait InputWriter: Send + Sync {
    fn press_key(&self, key: &str) -> Result<(), ToolsError>;
}
