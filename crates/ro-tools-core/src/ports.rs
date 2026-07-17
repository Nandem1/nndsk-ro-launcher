use crate::error::ToolsError;
use std::time::Instant;

pub trait MemoryReader: Send + Sync {
    fn read_u32(&self, address: u32) -> Result<u32, ToolsError>;

    /// Reads contiguous status values. Adapters may override this to use a
    /// single system call; the default keeps existing test adapters simple.
    fn read_u32_slice(&self, address: u32, len: usize) -> Result<Vec<u32>, ToolsError> {
        (0..len)
            .map(|index| self.read_u32(address + (index as u32 * 4)))
            .collect()
    }

    /// Null-terminated string (4RTools reads up to 40 bytes).
    fn read_string(&self, address: u32, max_len: usize) -> Result<String, ToolsError>;
}

pub trait KeyPressWriter: Send + Sync {
    fn press_key(&self, key: &str) -> Result<(), ToolsError>;
}

pub trait HeldKeyWriter: Send + Sync {
    fn key_down(&self, key: &str) -> Result<(), ToolsError>;
    fn key_up(&self, key: &str) -> Result<(), ToolsError>;
}

/// Writes the complete spam sequence as one non-interleavable command.
pub trait SpamCycleWriter: Send + Sync {
    /// Returns false when the cycle was deliberately skipped after its deadline.
    fn spam_cycle(&self, key: &str, deadline: Option<Instant>) -> Result<bool, ToolsError>;
}
