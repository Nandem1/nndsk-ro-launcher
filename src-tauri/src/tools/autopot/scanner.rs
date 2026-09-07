use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ro_tools_core::{map_label_matches, map_scan_needles, normalize_map_name, MemoryReader};
use ro_tools_linux::{
    capture_process_identity, find_all_writable_bytes, scan_writable_u32, verify_process_identity,
    ProcMemoryReader, ProcessIdentity,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelScanProgress {
    pub pid: u32,
    pub candidate_count: usize,
    pub confirmed: Option<DetectedLevelAddress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedLevelAddress {
    pub level_address: String,
    pub job_level_address: Option<String>,
    pub current_level: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapScanProgress {
    pub pid: u32,
    pub candidate_count: usize,
    pub confirmed: Option<DetectedMapAddress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedMapAddress {
    pub map_address: String,
    pub map_name: String,
}

const LEVEL_ANCHOR_WINDOW: u32 = 256 * 1024;
const PACKED_SPAN: u32 = 128;
const MAX_LEVEL: u32 = 300;
const MAX_MAP_LEN: usize = 40;

#[derive(Debug)]
enum ScanKind {
    Hp { last_value: u32 },
    Level { last_value: u32 },
    Map { last_map: String },
}

#[derive(Debug)]
struct ScanSession {
    identity: ProcessIdentity,
    candidates: Vec<u32>,
    kind: ScanKind,
    anchors: Vec<u32>,
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
                    kind: ScanKind::Hp {
                        last_value: current_hp,
                    },
                    anchors: Vec::new(),
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
                ScanState::Ready(session) if matches!(session.kind, ScanKind::Hp { last_value } if last_value != current_hp) => {
                    session
                }
                ScanState::Ready(session) if matches!(session.kind, ScanKind::Hp { .. }) => {
                    *state = ScanState::Ready(session);
                    return Err("El HP no cambió. Pierde o recupera HP antes de continuar".into());
                }
                ScanState::Ready(session) => {
                    *state = ScanState::Ready(session);
                    return Err("Primero inicia una búsqueda con el HP actual".into());
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
                kind: ScanKind::Hp {
                    last_value: current_hp,
                },
                anchors: Vec::new(),
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
        hp_base: Option<u32>,
    ) -> Result<DetectedNameAddress, String> {
        let character_name = character_name.trim().to_string();
        validate_character_name(&character_name)?;
        let identity = capture_process_identity(pid)
            .ok_or_else(|| "El proceso del juego ya no está disponible".to_string())?;
        let name_for_scan = character_name.clone();
        let found = tokio::task::spawn_blocking(move || {
            let mut needle = name_for_scan.into_bytes();
            needle.push(0);
            let candidates =
                find_all_writable_bytes(pid, &needle).map_err(|error| error.to_string())?;
            Ok::<_, String>(pick_character_name(candidates, hp_base))
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

    pub async fn begin_level(
        &self,
        pid: u32,
        current_level: u32,
        name_address: Option<u32>,
        hp_base: Option<u32>,
    ) -> Result<LevelScanProgress, String> {
        validate_level(current_level)?;
        let identity = capture_process_identity(pid)
            .ok_or_else(|| "El proceso del juego ya no está disponible".to_string())?;
        let generation = self.reserve_scan(true)?;
        let anchors: Vec<u32> = [name_address, hp_base].into_iter().flatten().collect();
        let scan_anchors = anchors.clone();

        let result = match tokio::task::spawn_blocking(move || {
            let candidates =
                scan_writable_u32(pid, current_level).map_err(|error| error.to_string())?;
            Ok::<_, String>(narrow_scan_candidates(candidates, &scan_anchors))
        })
        .await
        {
            Ok(result) => result,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(format!(
                    "El escáner de nivel terminó inesperadamente: {error}"
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
                    "No se encontró el nivel {current_level} cerca de HP/nombre en la memoria escribible"
                ))
            }
            Ok(candidates) => {
                let candidate_count = candidates.len();
                *state = ScanState::Ready(ScanSession {
                    identity,
                    candidates,
                    kind: ScanKind::Level {
                        last_value: current_level,
                    },
                    anchors,
                });
                Ok(LevelScanProgress {
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

    pub async fn refine_level(&self, current_level: u32) -> Result<LevelScanProgress, String> {
        validate_level(current_level)?;
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let session = {
            let mut state = self.lock()?;
            let previous = std::mem::replace(&mut *state, ScanState::Scanning { generation });
            match previous {
                ScanState::Ready(session) if matches!(session.kind, ScanKind::Level { last_value } if last_value != current_level) => {
                    session
                }
                ScanState::Ready(session) if matches!(session.kind, ScanKind::Level { .. }) => {
                    *state = ScanState::Ready(session);
                    return Err("El nivel no cambió. Sube de nivel antes de continuar".into());
                }
                ScanState::Ready(session) => {
                    *state = ScanState::Ready(session);
                    return Err("Primero inicia una búsqueda con el nivel actual".into());
                }
                ScanState::Idle => {
                    *state = ScanState::Idle;
                    return Err("Primero inicia una búsqueda con el nivel actual".into());
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
        let anchors = session.anchors.clone();
        let compare_anchors = anchors.clone();
        let result = match tokio::task::spawn_blocking(move || {
            let reader = ProcMemoryReader::open(pid).map_err(|error| error.to_string())?;
            let candidates = reader.refine_u32_candidates(&session.candidates, current_level);
            let chosen = resolve_after_compare(&candidates, &compare_anchors);
            let confirmed =
                chosen.and_then(|address| detect_level_address(&reader, address, current_level));
            Ok::<_, String>((candidates, confirmed))
        })
        .await
        {
            Ok(result) => result,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(format!(
                    "El escáner de nivel terminó inesperadamente: {error}"
                ));
            }
        };

        let (candidates, confirmed) = match result {
            Ok(found) => found,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(error);
            }
        };

        if candidates.is_empty() {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Err(format!(
                "Ninguna dirección candidata cambió al nivel {current_level}; vuelve a iniciar la búsqueda"
            ));
        }
        if let Some(confirmed) = confirmed {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Ok(LevelScanProgress {
                pid,
                candidate_count: 1,
                confirmed: Some(confirmed),
            });
        }

        let candidate_count = candidates.len();
        self.finish_if_current(
            generation,
            ScanState::Ready(ScanSession {
                identity,
                candidates,
                kind: ScanKind::Level {
                    last_value: current_level,
                },
                anchors,
            }),
        )?;
        Ok(LevelScanProgress {
            pid,
            candidate_count,
            confirmed: None,
        })
    }

    pub async fn begin_map(
        &self,
        pid: u32,
        map_name: String,
        name_address: Option<u32>,
        hp_base: Option<u32>,
        level_address: Option<u32>,
    ) -> Result<MapScanProgress, String> {
        let map_name = normalize_scan_map(&map_name)?;
        let identity = capture_process_identity(pid)
            .ok_or_else(|| "El proceso del juego ya no está disponible".to_string())?;
        let generation = self.reserve_scan(true)?;
        let needles = map_scan_needles(&map_name)
            .ok_or_else(|| "El nombre del mapa no es válido".to_string())?;
        let anchors: Vec<u32> = [name_address, hp_base, level_address]
            .into_iter()
            .flatten()
            .collect();
        let scan_anchors = anchors.clone();

        let result = match tokio::task::spawn_blocking(move || {
            let mut candidates = Vec::new();
            for needle in &needles {
                let found =
                    find_all_writable_bytes(pid, needle).map_err(|error| error.to_string())?;
                candidates.extend(found);
            }
            candidates.sort_unstable();
            candidates.dedup();
            Ok::<_, String>(narrow_scan_candidates(candidates, &scan_anchors))
        })
        .await
        {
            Ok(result) => result,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(format!(
                    "El escáner de mapa terminó inesperadamente: {error}"
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
                    "No se encontró '{map_name}' en la memoria escribible del cliente"
                ))
            }
            Ok(candidates) => {
                let candidate_count = candidates.len();
                *state = ScanState::Ready(ScanSession {
                    identity,
                    candidates,
                    kind: ScanKind::Map { last_map: map_name },
                    anchors,
                });
                Ok(MapScanProgress {
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

    pub async fn refine_map(&self, map_name: String) -> Result<MapScanProgress, String> {
        let map_name = normalize_scan_map(&map_name)?;
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let session = {
            let mut state = self.lock()?;
            let previous = std::mem::replace(&mut *state, ScanState::Scanning { generation });
            match previous {
                ScanState::Ready(session) if matches!(&session.kind, ScanKind::Map { last_map } if last_map != &map_name) => {
                    session
                }
                ScanState::Ready(session) if matches!(session.kind, ScanKind::Map { .. }) => {
                    *state = ScanState::Ready(session);
                    return Err("El mapa no cambió. Cambia de mapa antes de continuar".into());
                }
                ScanState::Ready(session) => {
                    *state = ScanState::Ready(session);
                    return Err("Primero inicia una búsqueda con el mapa actual".into());
                }
                ScanState::Idle => {
                    *state = ScanState::Idle;
                    return Err("Primero inicia una búsqueda con el mapa actual".into());
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
        let anchors = session.anchors.clone();
        let compare_anchors = anchors.clone();
        let previous_map = match &session.kind {
            ScanKind::Map { last_map } => last_map.clone(),
            _ => map_name.clone(),
        };
        let expected = map_name.clone();
        let result = match tokio::task::spawn_blocking(move || {
            let reader = ProcMemoryReader::open(pid).map_err(|error| error.to_string())?;
            let changed = session
                .candidates
                .iter()
                .copied()
                .filter_map(|address| {
                    let raw = reader.read_string(address, MAX_MAP_LEN).ok()?;
                    if !map_label_matches(&raw, &expected) {
                        return None;
                    }
                    if map_label_matches(&raw, &previous_map) {
                        return None;
                    }
                    Some((address, raw))
                })
                .collect::<Vec<_>>();
            let confirmed = resolve_map_after_compare(&changed, &compare_anchors).map(|address| {
                DetectedMapAddress {
                    map_address: format_address(address),
                    map_name: expected.clone(),
                }
            });
            let candidates: Vec<u32> = changed.into_iter().map(|(address, _)| address).collect();
            Ok::<_, String>((candidates, confirmed))
        })
        .await
        {
            Ok(result) => result,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(format!(
                    "El escáner de mapa terminó inesperadamente: {error}"
                ));
            }
        };

        let (candidates, confirmed) = match result {
            Ok(found) => found,
            Err(error) => {
                self.finish_if_current(generation, ScanState::Idle)?;
                return Err(error);
            }
        };

        if candidates.is_empty() {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Err(format!(
                "Ninguna dirección candidata cambió a '{map_name}'; vuelve a iniciar la búsqueda"
            ));
        }
        if let Some(confirmed) = confirmed {
            self.finish_if_current(generation, ScanState::Idle)?;
            return Ok(MapScanProgress {
                pid,
                candidate_count: 1,
                confirmed: Some(confirmed),
            });
        }

        let candidate_count = candidates.len();
        self.finish_if_current(
            generation,
            ScanState::Ready(ScanSession {
                identity,
                candidates,
                kind: ScanKind::Map { last_map: map_name },
                anchors,
            }),
        )?;
        Ok(MapScanProgress {
            pid,
            candidate_count,
            confirmed: None,
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

fn validate_level(current_level: u32) -> Result<(), String> {
    if !(1..=MAX_LEVEL).contains(&current_level) {
        return Err(format!("El nivel debe estar entre 1 y {MAX_LEVEL}"));
    }
    Ok(())
}

fn normalize_scan_map(raw: &str) -> Result<String, String> {
    normalize_map_name(raw).ok_or_else(|| "El nombre del mapa no es válido".to_string())
}

fn prefer_nearby_candidates(candidates: Vec<u32>, anchors: &[u32]) -> Vec<u32> {
    if anchors.is_empty() {
        return candidates;
    }
    let nearby: Vec<u32> = candidates
        .iter()
        .copied()
        .filter(|address| {
            anchors
                .iter()
                .any(|anchor| address.abs_diff(*anchor) <= LEVEL_ANCHOR_WINDOW)
        })
        .collect();
    if nearby.is_empty() {
        candidates
    } else {
        nearby
    }
}

fn drop_packed_clusters(candidates: Vec<u32>) -> Vec<u32> {
    if candidates.len() < 3 {
        return candidates;
    }
    let mut sorted = candidates.clone();
    sorted.sort_unstable();
    let isolated: Vec<u32> = sorted
        .iter()
        .copied()
        .filter(|address| {
            sorted
                .iter()
                .filter(|other| **other != *address && other.abs_diff(*address) <= PACKED_SPAN)
                .count()
                < 2
        })
        .collect();
    if isolated.is_empty() {
        candidates
    } else {
        isolated
    }
}

fn narrow_scan_candidates(candidates: Vec<u32>, anchors: &[u32]) -> Vec<u32> {
    drop_packed_clusters(prefer_nearby_candidates(candidates, anchors))
}

fn pick_nearest(candidates: &[u32], anchors: &[u32]) -> Option<u32> {
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(candidates[0]);
    }
    if anchors.is_empty() {
        return None;
    }
    candidates.iter().copied().min_by_key(|address| {
        anchors
            .iter()
            .map(|anchor| address.abs_diff(*anchor))
            .min()
            .unwrap_or(u32::MAX)
    })
}

fn resolve_after_compare(candidates: &[u32], anchors: &[u32]) -> Option<u32> {
    pick_nearest(candidates, anchors)
}

fn pick_character_name(candidates: Vec<u32>, hp_base: Option<u32>) -> Option<u32> {
    let anchors: Vec<u32> = hp_base.into_iter().collect();
    let narrowed = prefer_nearby_candidates(candidates, &anchors);
    pick_nearest(&narrowed, &anchors).or_else(|| narrowed.first().copied())
}

fn resolve_map_after_compare(changed: &[(u32, String)], anchors: &[u32]) -> Option<u32> {
    if changed.is_empty() {
        return None;
    }
    if changed.len() == 1 {
        return Some(changed[0].0);
    }
    changed
        .iter()
        .min_by_key(|(address, raw)| {
            let rsw_penalty = u32::from(!raw.to_ascii_lowercase().contains(".rsw"));
            let distance = anchors
                .iter()
                .map(|anchor| address.abs_diff(*anchor))
                .min()
                .unwrap_or(u32::MAX);
            (rsw_penalty, distance)
        })
        .map(|(address, _)| *address)
}

fn detect_level_address(
    reader: &ProcMemoryReader,
    level_address: u32,
    current_level: u32,
) -> Option<DetectedLevelAddress> {
    let value = reader.read_u32(level_address).ok()?;
    if value != current_level {
        return None;
    }
    let job_level_address = level_address.checked_add(8).and_then(|address| {
        let job = reader.read_u32(address).ok()?;
        (1..=MAX_LEVEL)
            .contains(&job)
            .then(|| format_address(address))
    });
    Some(DetectedLevelAddress {
        level_address: format_address(level_address),
        job_level_address,
        current_level,
    })
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

    #[test]
    fn level_must_be_in_the_presence_range() {
        assert!(validate_level(0).is_err());
        assert!(validate_level(1).is_ok());
        assert!(validate_level(99).is_ok());
        assert!(validate_level(300).is_ok());
        assert!(validate_level(301).is_err());
    }

    #[test]
    fn map_input_normalizes_like_the_presence_reader() {
        assert_eq!(
            normalize_scan_map("Prontera.rsw").as_deref(),
            Ok("prontera")
        );
        assert_eq!(normalize_scan_map("malaya").as_deref(), Ok("malaya"));
        assert!(normalize_scan_map("").is_err());
        assert!(normalize_scan_map("payon/map").is_err());
    }

    #[test]
    fn level_scan_prefers_candidates_near_name_or_hp() {
        let name = 0x0177_AE00;
        let far_static = 0x0040_1000;
        let near = name - 0x100;
        let kept = prefer_nearby_candidates(vec![far_static, near], &[name]);
        assert_eq!(kept, vec![near]);
    }

    #[test]
    fn map_refine_keeps_only_the_live_buffer() {
        let leftovers: Vec<(u32, String)> = [
            (0x1000u32, "prontera.rsw"),
            (0x2000, "prontera"),
            (0x3000, "izlude.rsw"),
        ]
        .into_iter()
        .filter(|(_, raw)| map_label_matches(raw, "izlude"))
        .map(|(address, raw)| (address, raw.to_string()))
        .collect();
        assert_eq!(
            resolve_map_after_compare(&leftovers, &[0x0177_AE00]),
            Some(0x3000)
        );
    }

    #[test]
    fn one_map_compare_resolves_lockstep_copies_near_the_character() {
        let name = 0x0177_AE00;
        let changed = vec![
            (0x0040_2000u32, "izlude".to_string()),
            (name - 0x80, "izlude.rsw".to_string()),
            (name + 0x200, "izlude".to_string()),
        ];
        assert_eq!(
            resolve_map_after_compare(&changed, &[name]),
            Some(name - 0x80)
        );
    }

    #[test]
    fn packed_map_tables_are_dropped_before_refine() {
        let isolated = 0x0177_7000;
        let kept = drop_packed_clusters(vec![0x1000, 0x1010, 0x1020, isolated]);
        assert_eq!(kept, vec![isolated]);
    }

    #[test]
    fn character_name_prefers_the_copy_near_hp() {
        let hp = 0x0177_8190;
        assert_eq!(
            pick_character_name(vec![0x0040_0100, hp + 0x2C70, 0x0200_0000], Some(hp)),
            Some(hp + 0x2C70)
        );
    }
}
