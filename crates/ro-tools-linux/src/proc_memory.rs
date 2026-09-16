use ro_tools_core::{MemoryReader, ToolsError};
use std::fs::{self, File};
use std::os::unix::fs::FileExt;
use std::sync::Mutex;
use thiserror::Error;

use crate::wine_process::ProcessIdentity;

const SCAN_CHUNK_SIZE: usize = 1024 * 1024;
const MAX_SCAN_CANDIDATES: usize = 2_000_000;

#[derive(Debug, Error)]
pub enum ProcMemoryError {
    #[error("failed to open /proc/{pid}/mem: {message}")]
    Open { pid: u32, message: String },
}

impl std::fmt::Display for MemoryReadDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "pid={}/{} addr=0x{:x} size={} vm_errno={} proc_mem_errno={} mem_open_errno={} ptrace_scope={} launcher_uid={} target_uid={} target_ppid={} descendant={} address_mapped={}",
            self.identity.pid,
            self.identity.start_time,
            self.address,
            self.size,
            format_errno(self.vm_errno),
            format_errno(self.proc_mem_errno),
            format_errno(self.mem_open_errno),
            opt_u32(self.ptrace_scope),
            self.launcher_uid,
            opt_u32(self.target_uid),
            opt_u32(self.target_ppid),
            self.descendant_of_ancestor
                .map(|v| if v { "true" } else { "false" })
                .unwrap_or("unknown"),
            self.address_mapped,
        )
    }
}

fn format_errno(errno: Option<i32>) -> String {
    errno
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "none".into())
}

#[derive(Debug, Clone)]
pub struct MemoryReadDiagnostic {
    pub identity: ProcessIdentity,
    pub address: u32,
    pub size: usize,
    pub vm_errno: Option<i32>,
    pub proc_mem_errno: Option<i32>,
    pub mem_open_errno: Option<i32>,
    pub ptrace_scope: Option<u32>,
    pub launcher_uid: u32,
    pub target_uid: Option<u32>,
    pub target_ppid: Option<u32>,
    pub descendant_of_ancestor: Option<bool>,
    pub address_mapped: bool,
}

#[derive(Debug)]
pub struct ProcMemoryReader {
    pid: u32,
    file: Mutex<Option<File>>,
    mem_open_errno: Option<i32>,
}

impl ProcMemoryReader {
    pub fn open(pid: u32) -> Result<Self, ProcMemoryError> {
        if fs::metadata(format!("/proc/{pid}")).is_err() {
            return Err(ProcMemoryError::Open {
                pid,
                message: "proceso no encontrado".into(),
            });
        }

        let mem_path = format!("/proc/{pid}/mem");
        let (file, mem_open_errno) = match File::open(&mem_path) {
            Ok(file) => (Some(file), None),
            Err(error) => (None, error.raw_os_error()),
        };
        Ok(Self {
            pid,
            file: Mutex::new(file),
            mem_open_errno,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn mem_open_errno(&self) -> Option<i32> {
        self.mem_open_errno
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

    pub fn refine_u32_candidates(&self, candidates: &[u32], value: u32) -> Vec<u32> {
        candidates
            .iter()
            .copied()
            .filter(|address| self.read_u32(*address).ok() == Some(value))
            .collect()
    }

    /// Intenta leer exactamente `buf.len()` bytes vía `process_vm_readv` sin fallback.
    pub fn try_vm_read(&self, address: u32, buf: &mut [u8]) -> Result<usize, i32> {
        read_via_vm(self.pid, address, buf)
    }

    /// Intenta leer vía `/proc/mem` sin usar `process_vm_readv`.
    pub fn try_proc_mem_read(&self, address: u32, buf: &mut [u8]) -> Result<usize, i32> {
        let guard = self.file.lock().map_err(|_| -1)?;
        let Some(file) = guard.as_ref() else {
            return Err(self.mem_open_errno.unwrap_or(libc::EACCES));
        };
        match file.read_at(buf, address as u64) {
            Ok(n) => Ok(n),
            Err(error) => Err(error.raw_os_error().unwrap_or(-1)),
        }
    }

    pub fn build_diagnostic(
        &self,
        identity: ProcessIdentity,
        address: u32,
        size: usize,
        vm_errno: Option<i32>,
        proc_mem_errno: Option<i32>,
        descendant_of_ancestor: Option<bool>,
    ) -> MemoryReadDiagnostic {
        MemoryReadDiagnostic {
            identity,
            address,
            size,
            vm_errno,
            proc_mem_errno,
            mem_open_errno: self.mem_open_errno,
            ptrace_scope: read_ptrace_scope(),
            launcher_uid: read_launcher_uid(),
            target_uid: read_process_uid(self.pid),
            target_ppid: crate::wine_process::read_ppid(self.pid).ok(),
            descendant_of_ancestor,
            address_mapped: self.address_mapped(address),
        }
    }
}

pub fn read_ptrace_scope() -> Option<u32> {
    fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope")
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

pub fn read_launcher_uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn read_process_uid(pid: u32) -> Option<u32> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse().ok());
        }
    }
    None
}

/// Primera región `rw` con al menos cuatro bytes dentro del espacio u32.
pub fn first_rw_u32_region(maps: &str) -> Option<(u32, u32)> {
    for (start, end) in parse_writable_regions(maps) {
        let size = end.saturating_sub(start);
        if size >= 4 && start <= u64::from(u32::MAX) {
            return Some((start as u32, size.min(u64::from(u32::MAX)) as u32));
        }
    }
    None
}

pub fn scan_writable_u32(pid: u32, value: u32) -> Result<Vec<u32>, ToolsError> {
    let reader =
        ProcMemoryReader::open(pid).map_err(|error| ToolsError::Other(error.to_string()))?;
    scan_writable_u32_with_reader(&reader, value)
}

pub fn scan_writable_u32_with_reader(
    reader: &ProcMemoryReader,
    value: u32,
) -> Result<Vec<u32>, ToolsError> {
    let pid = reader.pid();
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))
        .map_err(|error| ToolsError::Other(format!("no se pudo leer /proc/{pid}/maps: {error}")))?;
    let regions = parse_writable_regions(&maps);
    if regions.is_empty() {
        return Err(ToolsError::Other(
            "el proceso no expone regiones de memoria legibles y escribibles".into(),
        ));
    }

    let needle = value.to_le_bytes();
    let mut candidates = Vec::new();
    let mut buffer = vec![0u8; SCAN_CHUNK_SIZE];
    let mut successful_reads = 0usize;
    let mut last_error = None;

    for (region_start, region_end) in regions {
        let mut address = region_start;
        while address < region_end {
            let remaining = (region_end - address) as usize;
            let requested = remaining.min(buffer.len());
            let chunk = &mut buffer[..requested];
            let address_u32 = address as u32;
            match read_bytes_at(
                pid,
                address_u32,
                chunk,
                &reader.file,
                reader.mem_open_errno(),
            ) {
                Ok(read) if read >= 4 => {
                    successful_reads += 1;
                    scan_aligned_chunk(address_u32, &chunk[..read], &needle, &mut candidates);
                    if candidates.len() > MAX_SCAN_CANDIDATES {
                        return Err(ToolsError::Other(format!(
                            "el valor aparece en más de {MAX_SCAN_CANDIDATES} direcciones; usa un HP actual distinto de cero"
                        )));
                    }
                }
                Ok(_) => {}
                Err(error) => last_error = Some(error.to_string()),
            }
            address += requested as u64;
        }
    }

    if successful_reads == 0 {
        return Err(ToolsError::Other(last_error.unwrap_or_else(|| {
            "no se pudo leer ninguna región escribible del cliente".into()
        })));
    }
    Ok(candidates)
}

pub fn find_first_writable_bytes(pid: u32, needle: &[u8]) -> Result<Option<u32>, ToolsError> {
    Ok(find_all_writable_bytes(pid, needle)?.into_iter().next())
}

pub fn find_all_writable_bytes(pid: u32, needle: &[u8]) -> Result<Vec<u32>, ToolsError> {
    let reader =
        ProcMemoryReader::open(pid).map_err(|error| ToolsError::Other(error.to_string()))?;
    find_all_writable_bytes_with_reader(&reader, needle)
}

pub fn find_all_writable_bytes_with_reader(
    reader: &ProcMemoryReader,
    needle: &[u8],
) -> Result<Vec<u32>, ToolsError> {
    if needle.is_empty() {
        return Err(ToolsError::Other(
            "la cadena buscada no puede estar vacía".into(),
        ));
    }
    let pid = reader.pid();
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))
        .map_err(|error| ToolsError::Other(format!("no se pudo leer /proc/{pid}/maps: {error}")))?;
    let regions = parse_writable_regions(&maps);
    if regions.is_empty() {
        return Err(ToolsError::Other(
            "el proceso no expone regiones de memoria legibles y escribibles".into(),
        ));
    }

    let mut buffer = vec![0u8; SCAN_CHUNK_SIZE];
    let mut combined = Vec::with_capacity(SCAN_CHUNK_SIZE + needle.len().saturating_sub(1));
    let mut overlap = Vec::with_capacity(needle.len().saturating_sub(1));
    let mut successful_reads = 0usize;
    let mut last_error = None;
    let mut matches = Vec::new();

    for (region_start, region_end) in regions {
        overlap.clear();
        let mut address = region_start;
        while address < region_end {
            let remaining = (region_end - address) as usize;
            let requested = remaining.min(buffer.len());
            match read_bytes_at(
                pid,
                address as u32,
                &mut buffer[..requested],
                &reader.file,
                reader.mem_open_errno(),
            ) {
                Ok(read) if read > 0 => {
                    successful_reads += 1;
                    let overlap_len = overlap.len();
                    combined.clear();
                    combined.extend_from_slice(&overlap);
                    combined.extend_from_slice(&buffer[..read]);
                    for offset in find_all_subslices(&combined, needle) {
                        let match_address = address
                            .saturating_sub(overlap_len as u64)
                            .saturating_add(offset as u64);
                        if let Ok(address) = u32::try_from(match_address) {
                            matches.push(address);
                            if matches.len() > MAX_SCAN_CANDIDATES {
                                return Err(ToolsError::Other(format!(
                                    "la cadena aparece en más de {MAX_SCAN_CANDIDATES} direcciones"
                                )));
                            }
                        }
                    }

                    let keep = needle.len().saturating_sub(1).min(combined.len());
                    overlap.clear();
                    overlap.extend_from_slice(&combined[combined.len() - keep..]);
                }
                Ok(_) => overlap.clear(),
                Err(error) => {
                    overlap.clear();
                    last_error = Some(error.to_string());
                }
            }
            address += requested as u64;
        }
    }

    if successful_reads == 0 {
        return Err(ToolsError::Other(last_error.unwrap_or_else(|| {
            "no se pudo leer ninguna región escribible del cliente".into()
        })));
    }
    matches.sort_unstable();
    matches.dedup();
    Ok(matches)
}

fn find_all_subslices(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == needle).then_some(offset))
        .collect()
}

pub fn parse_writable_regions(maps: &str) -> Vec<(u64, u64)> {
    maps.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let range = fields.next()?;
            let permissions = fields.next()?;
            if !permissions.starts_with("rw") {
                return None;
            }
            let (start, end) = range.split_once('-')?;
            let start = u64::from_str_radix(start, 16).ok()?;
            let end = u64::from_str_radix(end, 16).ok()?;
            let address_space_end = u64::from(u32::MAX) + 1;
            let clipped_end = end.min(address_space_end);
            if start >= clipped_end || start > u64::from(u32::MAX) {
                return None;
            }
            Some((start, clipped_end))
        })
        .collect()
}

fn scan_aligned_chunk(start: u32, bytes: &[u8], needle: &[u8; 4], output: &mut Vec<u32>) {
    let alignment = ((4 - (start & 3)) & 3) as usize;
    if bytes.len() < alignment + 4 {
        return;
    }
    for offset in (alignment..=bytes.len() - 4).step_by(4) {
        if bytes[offset..offset + 4] == needle[..] {
            if let Some(address) = start.checked_add(offset as u32) {
                output.push(address);
            }
        }
    }
}

impl MemoryReader for ProcMemoryReader {
    fn read_u32(&self, address: u32) -> Result<u32, ToolsError> {
        read_u32_at(self.pid, address, &self.file, self.mem_open_errno)
    }

    fn read_string(&self, address: u32, max_len: usize) -> Result<String, ToolsError> {
        let mut buf = vec![0u8; max_len];
        let n = read_bytes_at(self.pid, address, &mut buf, &self.file, self.mem_open_errno)?;
        let end = buf[..n].iter().position(|&b| b == 0).unwrap_or(n);
        Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
    }

    fn read_u32_slice(&self, address: u32, len: usize) -> Result<Vec<u32>, ToolsError> {
        let mut bytes = vec![0u8; len * 4];
        let read = read_bytes_at(
            self.pid,
            address,
            &mut bytes,
            &self.file,
            self.mem_open_errno,
        )?;
        if read != bytes.len() {
            return Err(ToolsError::MemoryRead {
                address,
                message: format!("lectura HP/SP incompleta: {read} de {} bytes", bytes.len()),
            });
        }
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect())
    }
}

fn read_u32_at(
    pid: u32,
    address: u32,
    file: &Mutex<Option<File>>,
    mem_open_errno: Option<i32>,
) -> Result<u32, ToolsError> {
    let mut buf = [0u8; 4];
    read_bytes_at(pid, address, &mut buf, file, mem_open_errno)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_bytes_at(
    pid: u32,
    address: u32,
    buf: &mut [u8],
    file: &Mutex<Option<File>>,
    mem_open_errno: Option<i32>,
) -> Result<usize, ToolsError> {
    let vm_errno = match read_via_vm(pid, address, buf) {
        Ok(n) => return Ok(n),
        Err(errno) => errno,
    };

    let mut guard = file
        .lock()
        .map_err(|_| ToolsError::Other("memory lock poisoned".into()))?;
    let Some(file) = guard.as_mut() else {
        return Err(ToolsError::MemoryRead {
            address,
            message: format!(
                "process_vm_readv errno={vm_errno}; /proc/mem no abierto (mem_open_errno={})",
                mem_open_errno
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".into())
            ),
        });
    };

    file.read_at(buf, address as u64)
        .map_err(|e| ToolsError::MemoryRead {
            address,
            message: format!(
                "process_vm_readv errno={vm_errno}; /proc/mem: {e} (mem_open_errno={})",
                mem_open_errno
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".into())
            ),
        })
}

fn read_via_vm(pid: u32, address: u32, buf: &mut [u8]) -> Result<usize, i32> {
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
        Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1))
    } else {
        Ok(n as usize)
    }
}

pub fn vm_read_errno(pid: u32, address: u32, size: usize) -> Result<(), i32> {
    let mut buf = vec![0u8; size];
    match read_via_vm(pid, address, &mut buf) {
        Ok(n) if n == size => Ok(()),
        Ok(_) => Err(libc::EFAULT),
        Err(errno) => Err(errno),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_writable_regions_inside_the_32_bit_address_space() {
        let maps = concat!(
            "00400000-00401000 r--p 00000000 00:00 0\n",
            "01000000-01002000 rw-p 00000000 00:00 0\n",
            "7fff00000000-7fff00001000 rw-p 00000000 00:00 0\n",
        );
        assert_eq!(parse_writable_regions(maps), vec![(0x01000000, 0x01002000)]);
    }

    #[test]
    fn first_rw_u32_region_picks_first_eligible_mapping() {
        let maps = concat!(
            "00400000-00401000 r--p 00000000 00:00 0\n",
            "01000000-01002000 rw-p 00000000 00:00 0\n",
        );
        assert_eq!(first_rw_u32_region(maps), Some((0x01000000, 0x2000)));
    }

    #[test]
    fn exact_scan_preserves_four_byte_alignment() {
        let value = 13_619u32.to_le_bytes();
        let mut bytes = vec![0u8; 20];
        bytes[3..7].copy_from_slice(&value);
        bytes[8..12].copy_from_slice(&value);
        let mut found = Vec::new();

        scan_aligned_chunk(0x1000, &bytes, &value, &mut found);

        assert_eq!(found, vec![0x1008]);
    }

    #[test]
    fn scan_alignment_accounts_for_an_unaligned_chunk_start() {
        let value = 13_430u32.to_le_bytes();
        let mut bytes = vec![0u8; 12];
        bytes[3..7].copy_from_slice(&value);
        let mut found = Vec::new();

        scan_aligned_chunk(0x1001, &bytes, &value, &mut found);

        assert_eq!(found, vec![0x1004]);
    }

    #[test]
    fn byte_search_finds_an_exact_unaligned_string() {
        assert_eq!(
            find_all_subslices(b"xxNombrePJ\0yy", b"NombrePJ\0"),
            vec![2]
        );
        assert_eq!(
            find_all_subslices(b"xxNombrePJyy", b"NombrePJ\0"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn byte_search_collects_every_exact_match() {
        assert_eq!(
            find_all_subslices(b"prontera\0xxprontera\0", b"prontera\0"),
            vec![0, 11]
        );
        assert_eq!(
            find_all_subslices(b"izlude\0", b"prontera\0"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn open_fails_for_missing_process() {
        match ProcMemoryReader::open(999_999_999) {
            Err(ProcMemoryError::Open { pid, message }) => {
                assert_eq!(pid, 999_999_999);
                assert!(message.contains("no encontrado"));
            }
            Ok(_) => panic!("expected open to fail for missing pid"),
        }
    }

    #[test]
    fn open_self_process_does_not_fail() {
        let pid = std::process::id();
        let reader = ProcMemoryReader::open(pid).expect("self exists");
        assert_eq!(reader.pid(), pid);
    }

    #[test]
    fn try_vm_read_on_missing_pid_returns_errno_without_opening() {
        let reader = ProcMemoryReader::open(std::process::id()).expect("self exists");
        let mut buf = [0u8; 4];
        let errno = reader
            .try_vm_read(0x1234_5678, &mut buf)
            .expect_err("missing remote mapping should fail");
        assert_ne!(errno, 0);
    }

    #[test]
    fn read_only_mapping_in_maps_but_not_writable_regions() {
        let maps = concat!(
            "00400000-00401000 r--p 00000000 00:00 0\n",
            "01000000-01002000 rw-p 00000000 00:00 0\n",
        );
        let ro_addr = 0x0040_0004u32;
        assert!(address_in_maps_from_str(maps, ro_addr));
        assert_eq!(parse_writable_regions(maps), vec![(0x01000000, 0x01002000)]);
        assert!(!writable_regions_contain(maps, ro_addr));
    }

    fn address_in_maps_from_str(maps: &str, address: u32) -> bool {
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

    fn writable_regions_contain(maps: &str, address: u32) -> bool {
        let addr = address as u64;
        parse_writable_regions(maps)
            .iter()
            .any(|(start, end)| addr >= *start && addr < *end)
    }
}
