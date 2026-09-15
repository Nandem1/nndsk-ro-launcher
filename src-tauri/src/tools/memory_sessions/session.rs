use std::fs;
use std::sync::{Arc, Mutex};

use ro_tools_core::{MemoryReader, ToolsError};
use ro_tools_linux::{
    address_in_maps, capture_process_identity, find_all_writable_bytes_with_reader,
    first_rw_u32_region, is_descendant_of, scan_writable_u32_with_reader, verify_process_identity,
    ProcMemoryReader, ProcessIdentity,
};

use crate::models::memory::{MemoryAccess, ProfileMemory};

use super::classify::{classify_preflight, PreflightEvidence};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBackend {
    ProcessVmReadv,
    ProcMem,
    ProcessVmReadvWithProcMemFallback,
}

#[derive(Clone, Copy)]
pub enum MemoryAncestor {
    Supervisor(ProcessIdentity),
    Launcher(ProcessIdentity),
}

pub struct MemorySession {
    identity: ProcessIdentity,
    reader: Arc<ProcMemoryReader>,
    backend: MemoryBackend,
    access: Mutex<MemoryAccess>,
    #[allow(dead_code)]
    ancestor: ProcessIdentity,
}

fn probe_four_bytes(reader: &ProcMemoryReader, address: u32) -> Result<(), i32> {
    let mut probe = [0u8; 4];
    match reader.try_vm_read(address, &mut probe) {
        Ok(4) => Ok(()),
        Ok(_) => Err(libc::EFAULT),
        Err(errno) => Err(errno),
    }
}

fn probe_four_bytes_proc_mem(reader: &ProcMemoryReader, address: u32) -> Result<(), i32> {
    let mut probe = [0u8; 4];
    match reader.try_proc_mem_read(address, &mut probe) {
        Ok(4) => Ok(()),
        Ok(_) => Err(libc::EFAULT),
        Err(errno) => Err(errno),
    }
}

impl MemorySession {
    pub fn open(identity: ProcessIdentity, ancestor: MemoryAncestor) -> Result<Self, String> {
        if capture_process_identity(identity.pid).as_ref() != Some(&identity) {
            return Err("El proceso del cliente ya no es válido".into());
        }
        let reader = ProcMemoryReader::open(identity.pid)
            .map_err(|error| format!("No se pudo abrir memoria: {error}"))?;
        let ancestor_pid = match ancestor {
            MemoryAncestor::Supervisor(id) | MemoryAncestor::Launcher(id) => id,
        };
        let descendant = is_descendant_of(identity.pid, ancestor_pid.pid);
        let maps = fs::read_to_string(format!("/proc/{}/maps", identity.pid))
            .map_err(|error| format!("no se pudo leer maps: {error}"))?;
        let rw_region = first_rw_u32_region(&maps);
        let (vm_result, proc_mem_result) = if let Some((probe_address, _)) = rw_region {
            (
                probe_four_bytes(&reader, probe_address),
                probe_four_bytes_proc_mem(&reader, probe_address),
            )
        } else {
            (Err(libc::ENOENT), Err(libc::ENOENT))
        };
        let access = classify_preflight(PreflightEvidence {
            identity_alive: true,
            descendant_of_ancestor: descendant,
            has_rw_region: rw_region.is_some(),
            vm_result,
            proc_mem_result,
        });
        let backend = match access {
            MemoryAccess::ProcessVmReadv => MemoryBackend::ProcessVmReadv,
            MemoryAccess::ProcMem => MemoryBackend::ProcMem,
            _ => MemoryBackend::ProcessVmReadvWithProcMemFallback,
        };
        Ok(Self {
            identity,
            reader: Arc::new(reader),
            backend,
            access: Mutex::new(access),
            ancestor: ancestor_pid,
        })
    }

    #[allow(dead_code)]
    pub fn identity(&self) -> ProcessIdentity {
        self.identity
    }

    pub fn access(&self) -> MemoryAccess {
        *self
            .access
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn backend_label(&self) -> &'static str {
        match self.backend {
            MemoryBackend::ProcessVmReadv => "process_vm_readv",
            MemoryBackend::ProcMem => "proc_mem",
            MemoryBackend::ProcessVmReadvWithProcMemFallback => "process_vm_readv+proc_mem",
        }
    }

    pub fn profile_memory(&self, hp_base: Option<u32>) -> ProfileMemory {
        let hp_base = hp_base.filter(|value| *value != 0);
        let Some(hp_base) = hp_base else {
            return ProfileMemory::NotConfigured;
        };
        if !address_in_maps(self.identity.pid, hp_base) {
            return ProfileMemory::AddressUnmapped;
        }
        if self.probe_stats(hp_base).is_ok() {
            ProfileMemory::Valid
        } else {
            ProfileMemory::InvalidRead
        }
    }

    pub fn probe_stats(&self, hp_base: u32) -> Result<(u32, u32, u32, u32), ToolsError> {
        self.ensure_identity()?;
        let stats = self
            .reader
            .probe_stats(hp_base)
            .map_err(|error| self.enrich_memory_error(hp_base, 16, error))?;
        self.ensure_identity()?;
        Ok(stats)
    }

    pub fn scan_writable_u32(&self, value: u32) -> Result<Vec<u32>, ToolsError> {
        self.ensure_identity()?;
        let candidates = scan_writable_u32_with_reader(&self.reader, value)
            .map_err(|error| self.enrich_memory_error(0, 4, error))?;
        self.ensure_identity()?;
        Ok(candidates)
    }

    pub fn find_all_writable_bytes(&self, needle: &[u8]) -> Result<Vec<u32>, ToolsError> {
        self.ensure_identity()?;
        let candidates = find_all_writable_bytes_with_reader(&self.reader, needle)
            .map_err(|error| self.enrich_memory_error(0, needle.len().max(1), error))?;
        self.ensure_identity()?;
        Ok(candidates)
    }

    pub fn refine_u32_candidates(&self, candidates: &[u32], value: u32) -> Vec<u32> {
        candidates
            .iter()
            .copied()
            .filter(|address| self.read_u32(*address).ok() == Some(value))
            .collect()
    }

    fn enrich_memory_error(&self, address: u32, size: usize, error: ToolsError) -> ToolsError {
        match error {
            ToolsError::MemoryRead {
                address: _,
                message: _,
            } => {
                let mut probe = [0u8; 4];
                let vm_errno = self.reader.try_vm_read(address, &mut probe).err();
                let proc_mem_errno = self.reader.try_proc_mem_read(address, &mut probe).err();
                let diagnostic = self.reader.build_diagnostic(
                    self.identity,
                    address,
                    size,
                    vm_errno,
                    proc_mem_errno,
                    None,
                );
                ToolsError::MemoryRead {
                    address,
                    message: diagnostic.to_string(),
                }
            }
            other => other,
        }
    }

    fn ensure_identity(&self) -> Result<(), ToolsError> {
        if verify_process_identity(&self.identity) {
            Ok(())
        } else {
            if let Ok(mut access) = self.access.lock() {
                *access = MemoryAccess::ProcessExitedOrReused;
            }
            Err(ToolsError::MemoryRead {
                address: 0,
                message: "proceso terminado o PID reutilizado".into(),
            })
        }
    }
}

impl MemoryReader for MemorySession {
    fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
        self.ensure_identity()?;
        let value = self
            .reader
            .read_u32(address)
            .map_err(|error| self.enrich_memory_error(address, 4, error))?;
        self.ensure_identity()?;
        Ok(value)
    }

    fn read_string(&self, address: u32, max_len: usize) -> Result<String, ToolsError> {
        self.ensure_identity()?;
        let value = self
            .reader
            .read_string(address, max_len)
            .map_err(|error| self.enrich_memory_error(address, max_len, error))?;
        self.ensure_identity()?;
        Ok(value)
    }

    fn read_u32_slice(&self, address: u32, len: usize) -> Result<Vec<u32>, ToolsError> {
        self.ensure_identity()?;
        let value = self
            .reader
            .read_u32_slice(address, len)
            .map_err(|error| self.enrich_memory_error(address, len * 4, error))?;
        self.ensure_identity()?;
        Ok(value)
    }
}
