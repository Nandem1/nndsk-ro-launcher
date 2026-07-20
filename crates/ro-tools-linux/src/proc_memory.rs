use ro_tools_core::{MemoryReader, ToolsError};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
use thiserror::Error;

const SCAN_CHUNK_SIZE: usize = 1024 * 1024;
const MAX_SCAN_CANDIDATES: usize = 2_000_000;

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

    /// Conserva únicamente las direcciones que todavía contienen `value`.
    ///
    /// Esto permite hacer un unknown/exact-value scan incremental sin volver a recorrer todo el
    /// espacio de memoria del cliente.
    pub fn refine_u32_candidates(&self, candidates: &[u32], value: u32) -> Vec<u32> {
        candidates
            .iter()
            .copied()
            .filter(|address| self.read_u32(*address).ok() == Some(value))
            .collect()
    }
}

/// Busca un valor `u32` exacto, alineado a cuatro bytes, en todas las regiones legibles y
/// escribibles del proceso. Las estadísticas del cliente RO viven en memoria mutable, por lo que
/// excluir código y mappings de sólo lectura reduce mucho el costo y los falsos positivos.
pub fn scan_writable_u32(pid: u32, value: u32) -> Result<Vec<u32>, ToolsError> {
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))
        .map_err(|error| ToolsError::Other(format!("no se pudo leer /proc/{pid}/maps: {error}")))?;
    let regions = parse_writable_regions(&maps);
    if regions.is_empty() {
        return Err(ToolsError::Other(
            "el proceso no expone regiones de memoria legibles y escribibles".into(),
        ));
    }

    let reader =
        ProcMemoryReader::open(pid).map_err(|error| ToolsError::Other(error.to_string()))?;
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
            match read_bytes_at(pid, address_u32, chunk, &reader.file) {
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

/// Devuelve la primera aparición exacta de `needle` en memoria legible y escribible, recorriendo
/// los mappings en orden ascendente. Conserva un pequeño solapamiento entre chunks para no perder
/// cadenas que crucen el límite de lectura.
pub fn find_first_writable_bytes(pid: u32, needle: &[u8]) -> Result<Option<u32>, ToolsError> {
    if needle.is_empty() {
        return Err(ToolsError::Other(
            "la cadena buscada no puede estar vacía".into(),
        ));
    }
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))
        .map_err(|error| ToolsError::Other(format!("no se pudo leer /proc/{pid}/maps: {error}")))?;
    let regions = parse_writable_regions(&maps);
    if regions.is_empty() {
        return Err(ToolsError::Other(
            "el proceso no expone regiones de memoria legibles y escribibles".into(),
        ));
    }

    let reader =
        ProcMemoryReader::open(pid).map_err(|error| ToolsError::Other(error.to_string()))?;
    let mut buffer = vec![0u8; SCAN_CHUNK_SIZE];
    let mut combined = Vec::with_capacity(SCAN_CHUNK_SIZE + needle.len().saturating_sub(1));
    let mut overlap = Vec::with_capacity(needle.len().saturating_sub(1));
    let mut successful_reads = 0usize;
    let mut last_error = None;

    for (region_start, region_end) in regions {
        overlap.clear();
        let mut address = region_start;
        while address < region_end {
            let remaining = (region_end - address) as usize;
            let requested = remaining.min(buffer.len());
            match read_bytes_at(pid, address as u32, &mut buffer[..requested], &reader.file) {
                Ok(read) if read > 0 => {
                    successful_reads += 1;
                    let overlap_len = overlap.len();
                    combined.clear();
                    combined.extend_from_slice(&overlap);
                    combined.extend_from_slice(&buffer[..read]);
                    if let Some(offset) = find_subslice(&combined, needle) {
                        let match_address = address
                            .saturating_sub(overlap_len as u64)
                            .saturating_add(offset as u64);
                        if let Ok(address) = u32::try_from(match_address) {
                            return Ok(Some(address));
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
    Ok(None)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_writable_regions(maps: &str) -> Vec<(u64, u64)> {
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
        assert_eq!(find_subslice(b"xxNombrePJ\0yy", b"NombrePJ\0"), Some(2));
        assert_eq!(find_subslice(b"xxNombrePJyy", b"NombrePJ\0"), None);
    }
}
