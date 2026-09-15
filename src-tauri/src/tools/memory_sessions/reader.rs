use std::sync::Arc;

use ro_tools_core::{MemoryReader, ToolsError};

use super::MemorySession;

#[derive(Clone)]
pub struct SharedMemorySession(pub Arc<MemorySession>);

impl MemoryReader for SharedMemorySession {
    fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
        self.0.read_u32(address)
    }

    fn read_string(&self, address: u32, max_len: usize) -> Result<String, ToolsError> {
        self.0.read_string(address, max_len)
    }

    fn read_u32_slice(&self, address: u32, len: usize) -> Result<Vec<u32>, ToolsError> {
        self.0.read_u32_slice(address, len)
    }
}
