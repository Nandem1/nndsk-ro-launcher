use ro_tools_core::{MemoryReader, ToolsError};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcMemoryError {
    #[error("failed to open /proc/{pid}/mem: {message}")]
    Open { pid: u32, message: String },
}

pub struct ProcMemoryReader {
    pid: u32,
    file: Mutex<Option<File>>,
}

impl ProcMemoryReader {
    pub fn open(pid: u32) -> Result<Self, ProcMemoryError> {
        // Validar que el proceso existe; la lectura usa process_vm_readv o /proc/mem.
        if fs::metadata(format!("/proc/{pid}")).is_err() {
            return Err(ProcMemoryError::Open {
                pid,
                message: "proceso no encontrado".into(),
            });
        }

        let file = File::open(format!("/proc/{pid}/mem")).ok();
        Ok(Self {
            pid,
            file: Mutex::new(file),
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn address_mapped(&self, address: u32) -> bool {
        address_in_maps(self.pid, address)
    }

    pub fn probe_stats(&self, hp_base: u32) -> Result<(u32, u32, u32, u32), ToolsError> {
        let cur_hp = self.read_u32(hp_base)?;
        let max_hp = self.read_u32(hp_base + 4)?;
        let cur_sp = self.read_u32(hp_base + 8)?;
        let max_sp = self.read_u32(hp_base + 12)?;
        Ok((cur_hp, max_hp, cur_sp, max_sp))
    }
}

impl MemoryReader for ProcMemoryReader {
    fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
        read_u32_at(self.pid, address, &self.file)
    }

    fn read_string(&self, address: u32, max_len: usize) -> Result<String, ToolsError> {
        let mut buf = vec![0u8; max_len];
        let n = read_bytes_at(self.pid, address, &mut buf, &self.file)?;
        let end = buf[..n].iter().position(|&b| b == 0).unwrap_or(n);
        Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
    }

    fn read_u32_slice(&self, address: u32, len: usize) -> Result<Vec<u32>, ToolsError> {
        let mut bytes = vec![0u8; len * 4];
        let read = read_bytes_at(self.pid, address, &mut bytes, &self.file)?;
        if read != bytes.len() {
            return Err(ToolsError::MemoryRead {
                address,
                message: format!("lectura HP/SP incompleta: {read} de {} bytes", bytes.len()),
            });
        }
        Ok(bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("exact chunk")))
            .collect())
    }
}

fn read_u32_at(pid: u32, address: u32, file: &Mutex<Option<File>>) -> Result<u32, ToolsError> {
    let mut buf = [0u8; 4];
    read_bytes_at(pid, address, &mut buf, file)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_bytes_at(
    pid: u32,
    address: u32,
    buf: &mut [u8],
    file: &Mutex<Option<File>>,
) -> Result<usize, ToolsError> {
    if let Ok(n) = read_via_vm(pid, address, buf) {
        return Ok(n);
    }

    let mut guard = file
        .lock()
        .map_err(|_| ToolsError::Other("memory lock poisoned".into()))?;
    let Some(file) = guard.as_mut() else {
        return Err(ToolsError::MemoryRead {
            address,
            message: "sin permiso ptrace para /proc/mem (y process_vm_readv falló)".into(),
        });
    };

    file.seek(SeekFrom::Start(address as u64))
        .map_err(|e| ToolsError::MemoryRead {
            address,
            message: e.to_string(),
        })?;
    file.read(buf).map_err(|e| ToolsError::MemoryRead {
        address,
        message: e.to_string(),
    })
}

fn read_via_vm(pid: u32, address: u32, buf: &mut [u8]) -> Result<usize, ToolsError> {
    let local_iov = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let remote_iov = libc::iovec {
        iov_base: address as *mut libc::c_void,
        iov_len: buf.len(),
    };

    let n = unsafe { libc::process_vm_readv(pid as libc::pid_t, &local_iov, 1, &remote_iov, 1, 0) };

    if n < 0 {
        Err(ToolsError::MemoryRead {
            address,
            message: std::io::Error::last_os_error().to_string(),
        })
    } else {
        Ok(n as usize)
    }
}

pub fn address_in_maps(pid: u32, address: u32) -> bool {
    let Ok(maps) = fs::read_to_string(format!("/proc/{pid}/maps")) else {
        return false;
    };
    let addr = address as u64;
    for line in maps.lines() {
        let Some((range, _)) = line.split_once(' ') else {
            continue;
        };
        let Some((start, end)) = range.split_once('-') else {
            continue;
        };
        let Ok(start) = u64::from_str_radix(start, 16) else {
            continue;
        };
        let Ok(end) = u64::from_str_radix(end, 16) else {
            continue;
        };
        if addr >= start && addr < end {
            return true;
        }
    }
    false
}
