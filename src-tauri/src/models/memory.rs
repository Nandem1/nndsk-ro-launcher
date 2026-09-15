use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryAccess {
    ProcessVmReadv,
    ProcMem,
    NoReadableWritableRegion,
    OutsideSupervisor,
    YamaDenied,
    ProcessExitedOrReused,
    BackendError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileMemory {
    NotConfigured,
    AddressUnmapped,
    InvalidRead,
    Valid,
}

impl MemoryAccess {
    pub fn usable(self) -> bool {
        matches!(self, Self::ProcessVmReadv | Self::ProcMem)
    }
}
