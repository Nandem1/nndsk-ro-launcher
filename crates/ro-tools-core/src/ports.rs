use crate::error::ToolsError;

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

pub trait InputWriter: Send + Sync {
    fn press_key(&self, key: &str) -> Result<(), ToolsError>;

    /// Solo key-down (sin release). No-op por defecto — impls que lo necesitan deben sobreescribir.
    fn key_down(&self, key: &str) -> Result<(), ToolsError> {
        Err(ToolsError::Input {
            key: key.to_string(),
            message: "key_down no implementado".into(),
        })
    }

    /// Solo key-up (sin press). No-op por defecto — solo lo necesita el spammer con grab.
    fn key_up(&self, key: &str) -> Result<(), ToolsError> {
        let _ = key;
        Ok(())
    }

    /// Left mouse click (down + up). No-op by default for adapters that only emit keys.
    fn click_left(&self) -> Result<(), ToolsError> {
        Ok(())
    }
}
