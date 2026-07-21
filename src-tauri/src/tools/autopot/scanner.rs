use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ro_tools_linux::{
    capture_process_identity, find_first_writable_bytes, scan_writable_u32,
    verify_process_identity, ProcMemoryReader, ProcessIdentity,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryScanProgress {
    pub pid: u32,
    pub candidate_count: usize,
    pub confirmed: Option<DetectedMemoryLayout>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedMemoryLayout {
    pub hp_base: String,
    pub current_hp: u32,
    pub max_hp: u32,
    pub current_sp: u32,
    pub max_sp: u32,
    pub status_buffer: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedNameAddress {
    pub pid: u32,
    pub character_name: String,
    pub name_address: String,
}

#[derive(Debug)]
struct ScanSession {
    identity: ProcessIdentity,
    candidates: Vec<u32>,
    last_value: u32,
}

#[derive(Debug, Default)]
enum ScanState {
    #[default]
    Idle,
    Scanning {
        generation: u64,
    },
    Ready(ScanSession),
}

#[derive(Clone)]
pub struct MemoryScannerHandle {
    state: Arc<Mutex<ScanState>>,
    next_generation: Arc<AtomicU64>,
}

impl MemoryScannerHandle {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ScanState::Idle)),
            next_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub async fn begin(&self, pid: u32, current_hp: u32) -> Result<MemoryScanProgress, String> {
        validate_hp(current_hp)?;
        let identity = capture_process_identity(pid)
            .ok_or_else(|| "El proceso del juego ya no está disponible".to_string())?;
        let generation = self.reserve_scan(true)?;

        let result =
            match tokio::task::spawn_blocking(move || scan_writable_u32(pid, current_hp)).await {
                Ok(result) => result.map_err(|error| error.to_string()),
                Err(error) => {
                    self.finish_if_current(generation, ScanState::Idle)?;
                    return Err(format!(
                        "El escáner de memoria terminó inesperadamente: {error}"
                    ));
                }
            };

        let mut state = self.lock()?;
        if !matches!(*state, ScanState::Scanning { generation: active } if active == generation) {
            return Err("El escaneo fue cancelado".into());
        }

        match result {
            Ok(candidates) if candidates.is_empty() => {
                *state = ScanState::Idle;
                Err(format!(
                    "No se encontró el valor de HP {current_hp} en la memoria escribible del cliente"
                ))
            }
            Ok(candidates) => {
                let candidate_count = candidates.len();
                *state = ScanState::Ready(ScanSession {
                    identity,
                    candidates,
                    last_value: current_hp,
                });
                Ok(MemoryScanProgress {
                    pid,
                    candidate_count,
                    confirmed: None,
                })
            }
            Err(error) => {
                *state = ScanState::Idle;
                Err(error)
            }
        }
    }

    pub async fn refine(&self, current_hp: u32) -> Result<MemoryScanProgress, String> {
        validate_hp(current_hp)?;
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let session = {
            let mut state = self.lock()?;
            let previous = std::mem::replace(&mut *state, ScanState::Scanning { generation });
            match previous {
                ScanState::Ready(session) if session.last_value != current_hp => session,
                ScanState::Ready(session) => {
                    *state = ScanState::Ready(session);
                    return Err("El HP no cambió. Pierde o recupera HP antes de continuar".into());
                }
                ScanState::Idle => {
                    *state = ScanState::Idle;
                    return Err("Primero inicia una búsqueda con el HP actual".into());
                }
                ScanState::Scanning { generation } => {
                    *state = ScanState::Scanning { generation };
                    return Err("Ya hay un escaneo de memoria en curso".into());
                }
            }
        };

        if !verify_process_identity(&session.identity) {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Err("El proceso del juego cambió durante el escaneo; vuelve a empezar".into());
        }

        let pid = session.identity.pid;
        let identity = session.identity;
        let result = match tokio::task::spawn_blocking(move || {
            let reader = ProcMemoryReader::open(pid).map_err(|error| error.to_string())?;
            let candidates = reader.refine_u32_candidates(&session.candidates, current_hp);
            let layouts = candidates
                .iter()
                .filter_map(|address| detect_layout(&reader, *address, current_hp))
                .collect::<Vec<_>>();
            Ok::<_, String>((candidates, layouts))
        })
        .await
        {
            Ok(result) => result,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(format!(
                    "El escáner de memoria terminó inesperadamente: {error}"
                ));
            }
        };

        let (candidates, layouts) = match result {
            Ok(found) => found,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(error);
            }
        };

        if candidates.is_empty() {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Err(format!(
                "Ninguna dirección candidata cambió al HP {current_hp}; vuelve a iniciar la búsqueda"
            ));
        }
        if layouts.is_empty() {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Err(
                "Se encontró el HP, pero ninguna dirección tiene el bloque esperado HP máximo/SP. Este cliente usa otro layout"
                    .into(),
            );
        }
        if layouts.len() == 1 {
            let confirmed = layouts.into_iter().next();
            self.finish_if_current(generation, ScanState::Idle)?;
            return Ok(MemoryScanProgress {
                pid,
                candidate_count: 1,
                confirmed,
            });
        }

        let candidates = layouts
            .iter()
            .filter_map(|layout| parse_address(&layout.hp_base))
            .collect::<Vec<_>>();
        let candidate_count = candidates.len();
        self.finish_if_current(
            generation,
            ScanState::Ready(ScanSession {
                identity,
                candidates,
                last_value: current_hp,
            }),
        )?;
        Ok(MemoryScanProgress {
            pid,
            candidate_count,
            confirmed: None,
        })
    }

    pub async fn find_name(
        &self,
        pid: u32,
        character_name: String,
    ) -> Result<DetectedNameAddress, String> {
        let character_name = character_name.trim().to_string();
        validate_character_name(&character_name)?;
        let identity = capture_process_identity(pid)
            .ok_or_else(|| "El proceso del juego ya no está disponible".to_string())?;
        let name_for_scan = character_name.clone();
        let found = tokio::task::spawn_blocking(move || {
            let mut needle = name_for_scan.into_bytes();
            needle.push(0);
            find_first_writable_bytes(pid, &needle).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("El buscador del nombre terminó inesperadamente: {error}"))??;

        if !verify_process_identity(&identity) {
            return Err("El proceso del juego cambió durante la búsqueda; vuelve a empezar".into());
        }
        let address = found.ok_or_else(|| {
            format!(
                "No se encontró '{character_name}' como cadena exacta en la memoria escribible del cliente"
            )
        })?;
        Ok(DetectedNameAddress {
            pid,
            character_name,
            name_address: format_address(address),
        })
    }

    pub fn cancel(&self) {
        self.next_generation.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            *state = ScanState::Idle;
        }
    }

    fn reserve_scan(&self, require_idle: bool) -> Result<u64, String> {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let mut state = self.lock()?;
        if require_idle && !matches!(*state, ScanState::Idle) {
            return Err("Ya hay una búsqueda de memoria en curso; cancélala primero".into());
        }
        *state = ScanState::Scanning { generation };
        Ok(generation)
    }

    fn finish_if_current(&self, generation: u64, next: ScanState) -> Result<(), String> {
        let mut state = self.lock()?;
        if !matches!(*state, ScanState::Scanning { generation: active } if active == generation) {
            return Err("El escaneo fue cancelado".into());
        }
        *state = next;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ScanState>, String> {
        self.state
            .lock()
            .map_err(|_| "El estado del escáner de memoria está bloqueado".to_string())
    }
}

impl Default for MemoryScannerHandle {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_hp(current_hp: u32) -> Result<(), String> {
    if current_hp == 0 {
        return Err("El HP actual debe ser mayor que cero".into());
    }
    Ok(())
}

fn validate_character_name(character_name: &str) -> Result<(), String> {
    if character_name.is_empty() {
        return Err("El nombre del personaje no puede estar vacío".into());
    }
    if character_name.len() > 39 {
        return Err("El nombre del personaje no puede superar 39 bytes".into());
    }
    if character_name.chars().any(char::is_control) {
        return Err("El nombre del personaje contiene caracteres de control".into());
    }
    Ok(())
}

fn detect_layout(
    reader: &ProcMemoryReader,
    hp_base: u32,
    expected_hp: u32,
) -> Option<DetectedMemoryLayout> {
    hp_base.checked_add(0x474)?;
    let (current_hp, max_hp, current_sp, max_sp) = reader.probe_stats(hp_base).ok()?;
    if current_hp != expected_hp
        || max_hp < current_hp
        || max_hp == 0
        || max_sp < current_sp
        || max_sp == 0
    {
        return None;
    }
    Some(DetectedMemoryLayout {
        hp_base: format_address(hp_base),
        current_hp,
        max_hp,
        current_sp,
        max_sp,
        status_buffer: format_address(hp_base + 0x474),
    })
}

fn format_address(address: u32) -> String {
    format!("0x{address:08X}")
}

fn parse_address(address: &str) -> Option<u32> {
    u32::from_str_radix(address.strip_prefix("0x")?, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_round_trip_in_the_ui_format() {
        assert_eq!(format_address(0x10DCE10), "0x010DCE10");
        assert_eq!(parse_address("0x010DCE10"), Some(0x10DCE10));
    }

    #[test]
    fn zero_hp_is_rejected_before_scanning() {
        assert!(validate_hp(0).is_err());
        assert!(validate_hp(13_619).is_ok());
    }

    #[test]
    fn character_name_validation_matches_the_memory_reader_limit() {
        assert!(validate_character_name("NombrePJ").is_ok());
        assert!(validate_character_name("").is_err());
        assert!(validate_character_name(&"a".repeat(40)).is_err());
        assert!(validate_character_name("linea\nnueva").is_err());
    }
}
