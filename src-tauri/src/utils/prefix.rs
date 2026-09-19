use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::server::{PrefixMode, ServerConfig};

use super::paths::app_data_dir;

pub const LEGACY_PREFIX_MARKER: &str = ".ro-launcher-configured";
pub const PREFIX_MARKER: &str = ".ro-launcher-prefix.json";
pub const PREFIX_SCHEMA_VERSION: u32 = 2;
pub const PREFIX_SCHEMA_V3: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrefixScope {
    Shared,
    Isolated,
    Custom,
}

impl PrefixScope {
    pub fn as_str(self) -> &'static str {
        match self {
            PrefixScope::Shared => "shared",
            PrefixScope::Isolated => "isolated",
            PrefixScope::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixLocation {
    pub path: String,
    pub scope: PrefixScope,
    pub managed: bool,
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixManifest {
    pub schema_version: u32,
    pub scope: PrefixScope,
    pub server_id: Option<String>,
    pub runner_kind: String,
    pub runner_path: String,
    #[serde(default)]
    pub components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixFingerprintEnvelope {
    pub schema_version: u32,
    pub algorithm: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixManifestV3 {
    pub schema_version: u32,
    pub scope: PrefixScope,
    pub server_id: Option<String>,
    pub runner_kind: String,
    pub runner_path: String,
    #[serde(default)]
    pub components: Vec<String>,
    pub prefix_fingerprint: PrefixFingerprintEnvelope,
}

#[derive(Debug, Clone)]
pub enum StoredPrefixManifest {
    V2(PrefixManifest),
    V3(PrefixManifestV3),
}

impl StoredPrefixManifest {
    pub fn schema_version(&self) -> u32 {
        match self {
            Self::V2(manifest) => manifest.schema_version,
            Self::V3(manifest) => manifest.schema_version,
        }
    }

    pub fn server_id(&self) -> Option<&str> {
        match self {
            Self::V2(manifest) => manifest.server_id.as_deref(),
            Self::V3(manifest) => manifest.server_id.as_deref(),
        }
    }

    pub fn runner_kind(&self) -> &str {
        match self {
            Self::V2(manifest) => &manifest.runner_kind,
            Self::V3(manifest) => &manifest.runner_kind,
        }
    }

    pub fn runner_path(&self) -> &str {
        match self {
            Self::V2(manifest) => &manifest.runner_path,
            Self::V3(manifest) => &manifest.runner_path,
        }
    }

    pub fn components(&self) -> &[String] {
        match self {
            Self::V2(manifest) => &manifest.components,
            Self::V3(manifest) => &manifest.components,
        }
    }

    pub fn scope(&self) -> PrefixScope {
        match self {
            Self::V2(manifest) => manifest.scope,
            Self::V3(manifest) => manifest.scope,
        }
    }

    pub fn prefix_fingerprint_digest(&self) -> Option<&str> {
        match self {
            Self::V2(_) => None,
            Self::V3(manifest) => Some(manifest.prefix_fingerprint.digest.as_str()),
        }
    }

    pub fn as_v2(&self) -> Option<&PrefixManifest> {
        match self {
            Self::V2(manifest) => Some(manifest),
            Self::V3(_) => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PrefixHealth {
    pub structure_ok: bool,
    pub configured: bool,
    pub legacy_marker: bool,
    pub manifest: Option<StoredPrefixManifest>,
    pub issues: Vec<String>,
}

pub fn prefix_path() -> String {
    app_data_dir().join("prefix").to_string_lossy().to_string()
}

pub fn isolated_prefix_root() -> PathBuf {
    app_data_dir().join("prefixes")
}

pub fn isolated_prefix_path(server_id: &str) -> String {
    isolated_prefix_root()
        .join(format!("{:016x}", stable_id_hash(server_id)))
        .to_string_lossy()
        .to_string()
}

/// Prefix estable para una combinación exacta servidor/runner.
///
/// Impide abrir con un runner un entorno creado por otro y permite perfiles de compatibilidad.
pub fn isolated_prefix_path_for_runner(server_id: &str, runner_path: &str) -> String {
    isolated_prefix_root()
        .join(format!(
            "{:016x}-{:016x}",
            stable_id_hash(server_id),
            stable_id_hash(runner_path)
        ))
        .to_string_lossy()
        .to_string()
}

fn server_path_token16(server_id: &str) -> String {
    use sha2::{Digest, Sha256};
    const DOMAIN: &[u8] = b"ro-launcher/server-path/v1\0";
    let mut payload = Vec::new();
    payload.extend_from_slice(DOMAIN);
    let bytes = server_id.as_bytes();
    payload.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    payload.extend_from_slice(bytes);
    let digest = Sha256::digest(&payload);
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

/// Primeros 24 hex del digest completo (96 bits). El path v3 sólo los expone; el manifiesto guarda los 64.
pub fn v3_path_digest_prefix24(prefix_fingerprint_digest_hex: &str) -> &str {
    prefix_fingerprint_digest_hex
        .get(..24)
        .unwrap_or(prefix_fingerprint_digest_hex)
}

#[allow(dead_code)]
pub fn digests_share_v3_path_suffix(left_hex: &str, right_hex: &str) -> bool {
    v3_path_digest_prefix24(left_hex) == v3_path_digest_prefix24(right_hex)
}

pub fn isolated_prefix_path_v3(server_id: &str, prefix_fingerprint_digest_hex: &str) -> String {
    let token16 = server_path_token16(server_id);
    let digest24 = v3_path_digest_prefix24(prefix_fingerprint_digest_hex);
    isolated_prefix_root()
        .join(format!("{token16}-v3-{digest24}"))
        .to_string_lossy()
        .to_string()
}

pub fn is_v3_managed_prefix_path(path: &Path, server_id: &str) -> bool {
    if path.parent() != Some(isolated_prefix_root().as_path()) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let token16 = server_path_token16(server_id);
    let prefix = format!("{token16}-v3-");
    let Some(suffix) = name.strip_prefix(&prefix) else {
        return false;
    };
    suffix.len() == 24 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Compatibilidad temporal para call sites legacy. La resolución completa por servidor vive en
/// `resolve_server_prefix` una vez que se dispone del modo shared/isolated/custom.
#[allow(dead_code)]
pub fn effective_prefix(wine_prefix: Option<String>) -> String {
    wine_prefix.unwrap_or_else(prefix_path)
}

pub fn resolve_server_prefix(server: Option<&ServerConfig>) -> Result<PrefixLocation, String> {
    let runner = server.and_then(|server| server.runner.as_deref());
    resolve_server_prefix_with_runner(server, runner)
}

pub fn resolve_server_prefix_with_runner(
    server: Option<&ServerConfig>,
    effective_runner: Option<&str>,
) -> Result<PrefixLocation, String> {
    let Some(server) = server else {
        return Ok(PrefixLocation {
            path: prefix_path(),
            scope: PrefixScope::Shared,
            managed: true,
            server_id: None,
        });
    };

    let runner_identity = effective_runner.map(|runner| {
        std::fs::canonicalize(runner)
            .unwrap_or_else(|_| PathBuf::from(runner))
            .to_string_lossy()
            .to_string()
    });

    match server.effective_prefix_mode() {
        PrefixMode::Shared => Ok(PrefixLocation {
            path: prefix_path(),
            scope: PrefixScope::Shared,
            managed: true,
            server_id: None,
        }),
        PrefixMode::Isolated => Ok(PrefixLocation {
            path: runner_identity
                .as_deref()
                .map(|runner| isolated_prefix_path_for_runner(&server.id, runner))
                .unwrap_or_else(|| isolated_prefix_path(&server.id)),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some(server.id.clone()),
        }),
        PrefixMode::Custom => {
            let path = server
                .wine_prefix
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| "El modo custom requiere una ruta WINEPREFIX".to_string())?;
            Ok(PrefixLocation {
                path: path.to_string(),
                scope: PrefixScope::Custom,
                managed: false,
                server_id: Some(server.id.clone()),
            })
        }
    }
}

pub fn prefix_marker_path(prefix_path: &str) -> PathBuf {
    Path::new(prefix_path).join(PREFIX_MARKER)
}

pub fn legacy_prefix_marker_path(prefix_path: &str) -> PathBuf {
    Path::new(prefix_path).join(LEGACY_PREFIX_MARKER)
}

pub fn inspect_prefix(prefix_path: &str) -> PrefixHealth {
    let root = Path::new(prefix_path);
    let mut health = PrefixHealth::default();
    if !root.is_dir() {
        health
            .issues
            .push("El entorno todavía no existe".to_string());
        return health;
    }

    for (relative, label) in [
        ("drive_c/windows", "drive_c/windows"),
        ("system.reg", "system.reg"),
        ("user.reg", "user.reg"),
        ("dosdevices/c:", "dosdevices/c:"),
    ] {
        if !root.join(relative).exists() {
            health.issues.push(format!("Falta {label} en el entorno"));
        }
    }
    health.structure_ok = health.issues.is_empty();

    let marker = prefix_marker_path(prefix_path);
    if marker.is_file() {
        match std::fs::read_to_string(&marker) {
            Ok(json) => match parse_stored_prefix_manifest(&json) {
                Ok(manifest) => health.manifest = Some(manifest),
                Err(_) => {
                    if let Ok(probe) = serde_json::from_str::<SchemaProbe>(&json) {
                        if probe.schema_version != PREFIX_SCHEMA_VERSION
                            && probe.schema_version != PREFIX_SCHEMA_V3
                        {
                            health.issues.push(format!(
                                "El manifiesto usa un schema incompatible (esperado {PREFIX_SCHEMA_VERSION} o {PREFIX_SCHEMA_V3})"
                            ));
                        } else {
                            health
                                .issues
                                .push("El manifiesto del entorno está dañado".to_string());
                        }
                    } else {
                        health
                            .issues
                            .push("El manifiesto del entorno está dañado".to_string());
                    }
                }
            },
            Err(_) => health
                .issues
                .push("El manifiesto del entorno está dañado".to_string()),
        }
    } else if legacy_prefix_marker_path(prefix_path).is_file() {
        health.legacy_marker = true;
        health.issues.push(
            "Entorno legacy: se validó su estructura, pero aún no registra el runner que lo creó"
                .to_string(),
        );
    } else {
        health
            .issues
            .push("El entorno no tiene manifiesto del launcher".to_string());
    }

    health.configured = health.structure_ok && (health.manifest.is_some() || health.legacy_marker);
    if let Some(manifest) = &health.manifest {
        let schema = manifest.schema_version();
        if schema != PREFIX_SCHEMA_VERSION && schema != PREFIX_SCHEMA_V3 {
            health.issues.push(format!(
                "El manifiesto usa un schema incompatible (esperado {PREFIX_SCHEMA_VERSION} o {PREFIX_SCHEMA_V3})"
            ));
        }
    }
    health
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaProbe {
    schema_version: u32,
}

fn parse_stored_prefix_manifest(json: &str) -> Result<StoredPrefixManifest, String> {
    let probe: SchemaProbe = serde_json::from_str(json).map_err(|error| error.to_string())?;
    match probe.schema_version {
        PREFIX_SCHEMA_VERSION => serde_json::from_str::<PrefixManifest>(json)
            .map(StoredPrefixManifest::V2)
            .map_err(|error| error.to_string()),
        PREFIX_SCHEMA_V3 => serde_json::from_str::<PrefixManifestV3>(json)
            .map(StoredPrefixManifest::V3)
            .map_err(|error| error.to_string()),
        _ => Err("unsupported schema".to_string()),
    }
}

pub fn write_prefix_manifest(prefix_path: &str, manifest: &PrefixManifest) -> Result<(), String> {
    if manifest.schema_version != PREFIX_SCHEMA_VERSION {
        return Err("Versión de manifiesto de prefix inválida".to_string());
    }
    std::fs::create_dir_all(prefix_path).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    let destination = prefix_marker_path(prefix_path);
    if destination.is_symlink() {
        return Err("El manifiesto del entorno no puede ser un symlink".to_string());
    }
    let temporary = Path::new(prefix_path).join(format!(
        ".ro-launcher-prefix.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file.write_all(&json).and_then(|_| file.sync_all()) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    std::fs::rename(&temporary, &destination).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        error.to_string()
    })
}

pub fn write_prefix_manifest_v3(
    prefix_path: &str,
    manifest: &PrefixManifestV3,
) -> Result<(), String> {
    if manifest.schema_version != PREFIX_SCHEMA_V3 {
        return Err("Versión de manifiesto de prefix inválida".to_string());
    }
    std::fs::create_dir_all(prefix_path).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(manifest).map_err(|error| error.to_string())?;
    let destination = prefix_marker_path(prefix_path);
    if destination.is_symlink() {
        return Err("El manifiesto del entorno no puede ser un symlink".to_string());
    }
    let temporary = Path::new(prefix_path).join(format!(
        ".ro-launcher-prefix.tmp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file.write_all(&json).and_then(|_| file.sync_all()) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    std::fs::rename(&temporary, &destination).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        error.to_string()
    })
}

pub fn manifest_matches_runner(
    manifest: &PrefixManifest,
    expected_kind: &str,
    expected_path: &str,
) -> bool {
    stored_manifest_matches_runner(
        &StoredPrefixManifest::V2(manifest.clone()),
        expected_kind,
        expected_path,
    )
}

pub fn stored_manifest_matches_runner(
    manifest: &StoredPrefixManifest,
    expected_kind: &str,
    expected_path: &str,
) -> bool {
    manifest.runner_kind() == expected_kind
        && canonical_or_original(manifest.runner_path()) == canonical_or_original(expected_path)
}

#[allow(dead_code)]
pub fn manifest_matches_location(manifest: &PrefixManifest, location: &PrefixLocation) -> bool {
    manifest_matches_stored_location(&StoredPrefixManifest::V2(manifest.clone()), location)
}

pub fn manifest_matches_stored_location(
    manifest: &StoredPrefixManifest,
    location: &PrefixLocation,
) -> bool {
    if !location.managed {
        return true;
    }
    manifest.scope() == location.scope
        && match location.scope {
            PrefixScope::Shared => manifest.server_id().is_none(),
            PrefixScope::Isolated => manifest.server_id() == location.server_id.as_deref(),
            PrefixScope::Custom => true,
        }
}

pub fn is_dxvk_installed(prefix_path: &str) -> bool {
    let root = Path::new(prefix_path).join("drive_c/windows");
    let system32 = root.join("system32/d3d9.dll");
    let syswow64 = root.join("syswow64");
    if syswow64.is_dir() {
        system32.is_file() && syswow64.join("d3d9.dll").is_file()
    } else {
        system32.is_file()
    }
}

pub fn proton_vkd3d_companions_available(prefix_path: &str, proton_root: &Path) -> bool {
    const DLLS: [&str; 3] = [
        "libvkd3d-1.dll",
        "libvkd3d-shader-1.dll",
        "libvkd3d-utils-1.dll",
    ];
    let prefix_windows = Path::new(prefix_path).join("drive_c/windows");
    let runner_vkd3d = proton_root.join("files/lib/vkd3d");

    ["system32", "syswow64"].iter().all(|arch_dir| {
        let prefix_dir = prefix_windows.join(arch_dir);
        let runner_arch = if *arch_dir == "system32" {
            runner_vkd3d.join("x86_64-windows")
        } else {
            runner_vkd3d.join("i386-windows")
        };
        DLLS.iter()
            .all(|dll| prefix_dir.join(dll).is_file() || runner_arch.join(dll).is_file())
    })
}

pub fn proton_runner_vkd3d_companions_available(proton_root: &Path) -> bool {
    const DLLS: [&str; 3] = [
        "libvkd3d-1.dll",
        "libvkd3d-shader-1.dll",
        "libvkd3d-utils-1.dll",
    ];
    let runner_vkd3d = proton_root.join("files/lib/vkd3d");

    ["x86_64-windows", "i386-windows"].iter().all(|arch_dir| {
        DLLS.iter()
            .all(|dll| runner_vkd3d.join(arch_dir).join(dll).is_file())
    })
}

pub fn ensure_managed_reset_allowed(location: &PrefixLocation) -> Result<(), String> {
    if !location.managed || location.scope == PrefixScope::Custom {
        return Err("Por seguridad, el launcher no elimina WINEPREFIX personalizados".to_string());
    }
    ensure_managed_path_safe(location)?;
    let path = Path::new(&location.path);

    if path.exists()
        && path
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some())
    {
        let health = inspect_prefix(&location.path);
        if health.legacy_marker && health.structure_ok {
            return Ok(());
        }
        let manifest = health.manifest.ok_or_else(|| {
            "El entorno administrado no tiene un manifiesto válido; no se eliminará".to_string()
        })?;
        if manifest.schema_version() == 0
            || manifest.schema_version() > PREFIX_SCHEMA_V3
            || !manifest_matches_stored_location(&manifest, location)
        {
            return Err(
                "El manifiesto no pertenece al servidor seleccionado; no se eliminará".to_string(),
            );
        }
    }
    Ok(())
}

pub fn ensure_managed_path_safe(location: &PrefixLocation) -> Result<(), String> {
    if !location.managed || location.scope == PrefixScope::Custom {
        return Ok(());
    }
    let path = Path::new(&location.path);
    if path.is_symlink() {
        return Err("No se puede usar un entorno administrado mediante symlink".to_string());
    }
    if path.exists() && !path.is_dir() {
        return Err("La ruta del entorno administrado no es un directorio".to_string());
    }
    if location.scope == PrefixScope::Isolated && isolated_prefix_root().is_symlink() {
        return Err("La raíz de entornos aislados no puede ser un symlink".to_string());
    }

    let expected = match location.scope {
        PrefixScope::Shared => PathBuf::from(prefix_path()),
        PrefixScope::Isolated => {
            let server_id = location
                .server_id
                .as_deref()
                .ok_or_else(|| "El entorno aislado no tiene serverId".to_string())?;
            let legacy = PathBuf::from(isolated_prefix_path(server_id));
            if path == legacy
                || is_runner_scoped_prefix_path(path, server_id)
                || is_v3_managed_prefix_path(path, server_id)
            {
                path.to_path_buf()
            } else {
                legacy
            }
        }
        PrefixScope::Custom => unreachable!(),
    };
    if path != expected {
        return Err("La ruta del entorno no coincide con la ruta administrada".to_string());
    }
    if prefix_marker_path(&location.path).is_symlink() {
        return Err("El manifiesto del entorno no puede ser un symlink".to_string());
    }
    Ok(())
}

fn is_runner_scoped_prefix_path(path: &Path, server_id: &str) -> bool {
    if path.parent() != Some(isolated_prefix_root().as_path()) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let prefix = format!("{:016x}-", stable_id_hash(server_id));
    let Some(runner_hash) = name.strip_prefix(&prefix) else {
        return false;
    };
    runner_hash.len() == 16 && runner_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn ensure_custom_setup_allowed(location: &PrefixLocation) -> Result<(), String> {
    if location.scope != PrefixScope::Custom {
        return Ok(());
    }
    let root = Path::new(&location.path);
    if root.is_symlink() {
        return Err("El WINEPREFIX personalizado no puede ser un symlink".to_string());
    }
    if prefix_marker_path(&location.path).is_symlink() {
        return Err("El manifiesto del entorno no puede ser un symlink".to_string());
    }
    if root.is_dir()
        && root
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some())
        && !inspect_prefix(&location.path).structure_ok
    {
        return Err(
            "La ruta personalizada contiene datos, pero no parece un WINEPREFIX; no se modificará"
                .to_string(),
        );
    }
    Ok(())
}

fn stable_id_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn canonical_or_original(path: &str) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PrefixManifestFixtures {
        valid: PrefixManifest,
        future_schema: PrefixManifest,
        runner_mismatch: PrefixManifest,
        corrupt: String,
    }

    fn manifest_fixtures() -> PrefixManifestFixtures {
        serde_json::from_str(include_str!(
            "../../../contract-fixtures/runtime-prefix-manifests.json"
        ))
        .unwrap()
    }

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_prefix(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ro-launcher-prefix-health-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn create_structure(root: &Path) {
        std::fs::create_dir_all(root.join("drive_c/windows/system32")).unwrap();
        std::fs::create_dir_all(root.join("dosdevices")).unwrap();
        std::fs::write(root.join("system.reg"), "reg").unwrap();
        std::fs::write(root.join("user.reg"), "reg").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("../drive_c", root.join("dosdevices/c:")).unwrap();
    }

    #[test]
    fn malicious_server_ids_never_escape_managed_root() {
        let root = isolated_prefix_root();
        for id in ["../../escape", "/tmp/escape", "servidor 💣"] {
            let path = PathBuf::from(isolated_prefix_path(id));
            assert_eq!(path.parent(), Some(root.as_path()));
        }
    }

    #[test]
    fn runner_profiles_use_distinct_safe_prefixes() {
        let first = isolated_prefix_path_for_runner("sakura", "/opt/wine-7.16/bin/wine");
        let second = isolated_prefix_path_for_runner("sakura", "/opt/wine-current/bin/wine");
        assert_ne!(first, second);

        for path in [first, second] {
            let location = PrefixLocation {
                path,
                scope: PrefixScope::Isolated,
                managed: true,
                server_id: Some("sakura".to_string()),
            };
            assert!(ensure_managed_path_safe(&location).is_ok());
        }
    }

    #[test]
    fn marker_alone_does_not_hide_a_broken_structure() {
        let root = test_prefix("broken");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(LEGACY_PREFIX_MARKER), "configured").unwrap();
        let health = inspect_prefix(root.to_str().unwrap());
        assert!(!health.configured);
        assert!(!health.structure_ok);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn valid_legacy_prefix_is_detected_for_safe_rearm() {
        let root = test_prefix("legacy");
        create_structure(&root);
        std::fs::write(root.join(LEGACY_PREFIX_MARKER), "configured").unwrap();
        let health = inspect_prefix(root.to_str().unwrap());
        assert!(health.configured);
        assert!(health.legacy_marker);
        assert!(!health.issues.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dxvk_requires_both_architectures_in_wow64_prefix() {
        let root = test_prefix("dxvk");
        std::fs::create_dir_all(root.join("drive_c/windows/system32")).unwrap();
        std::fs::create_dir_all(root.join("drive_c/windows/syswow64")).unwrap();
        std::fs::write(root.join("drive_c/windows/system32/d3d9.dll"), "x64").unwrap();
        assert!(!is_dxvk_installed(root.to_str().unwrap()));
        std::fs::write(root.join("drive_c/windows/syswow64/d3d9.dll"), "x86").unwrap();
        assert!(is_dxvk_installed(root.to_str().unwrap()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reset_preflight_does_not_borrow_vkd3d_from_the_old_prefix() {
        let prefix = test_prefix("vkd3d-prefix");
        let proton = test_prefix("vkd3d-runner");
        for arch in ["system32", "syswow64"] {
            let dir = prefix.join("drive_c/windows").join(arch);
            std::fs::create_dir_all(&dir).unwrap();
            for dll in [
                "libvkd3d-1.dll",
                "libvkd3d-shader-1.dll",
                "libvkd3d-utils-1.dll",
            ] {
                std::fs::write(dir.join(dll), b"prefix copy").unwrap();
            }
        }

        assert!(proton_vkd3d_companions_available(
            prefix.to_str().unwrap(),
            &proton,
        ));
        assert!(!proton_runner_vkd3d_companions_available(&proton));
        std::fs::remove_dir_all(prefix).unwrap();
    }

    #[test]
    fn manifest_must_match_the_managed_server_location() {
        let manifest = PrefixManifest {
            schema_version: PREFIX_SCHEMA_VERSION,
            scope: PrefixScope::Isolated,
            server_id: Some("server-a".to_string()),
            runner_kind: "wine".to_string(),
            runner_path: "/usr/bin/wine".to_string(),
            components: Vec::new(),
        };
        let location = PrefixLocation {
            path: isolated_prefix_path("server-b"),
            scope: PrefixScope::Isolated,
            managed: true,
            server_id: Some("server-b".to_string()),
        };
        assert!(!manifest_matches_location(&manifest, &location));
    }

    #[test]
    fn prefix_manifest_fixtures_cover_valid_corrupt_future_and_runner_mismatch() {
        let fixtures = manifest_fixtures();
        let root = test_prefix("manifest-fixtures");
        create_structure(&root);

        std::fs::write(
            root.join(PREFIX_MARKER),
            serde_json::to_vec(&fixtures.valid).unwrap(),
        )
        .unwrap();
        let valid = inspect_prefix(root.to_str().unwrap());
        assert!(valid.configured);
        assert!(valid.issues.is_empty());
        let manifest = valid.manifest.as_ref().unwrap().as_v2().unwrap();
        assert!(manifest_matches_runner(
            manifest,
            "wine",
            "/opt/portable-wine/bin/wine"
        ));

        std::fs::write(root.join(PREFIX_MARKER), fixtures.corrupt).unwrap();
        let corrupt = inspect_prefix(root.to_str().unwrap());
        assert!(corrupt.manifest.is_none());
        assert!(corrupt
            .issues
            .iter()
            .any(|issue| issue.contains("manifiesto del entorno está dañado")));

        std::fs::write(
            root.join(PREFIX_MARKER),
            serde_json::to_vec(&fixtures.future_schema).unwrap(),
        )
        .unwrap();
        let future = inspect_prefix(root.to_str().unwrap());
        assert!(future
            .issues
            .iter()
            .any(|issue| issue.contains("schema incompatible")));

        assert!(!manifest_matches_runner(
            &fixtures.runner_mismatch,
            "wine",
            "/opt/portable-wine/bin/wine"
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn golden_managed_proton_path24_matches_plan() {
        const GOLDEN: &str = "a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270";
        assert_eq!(v3_path_digest_prefix24(GOLDEN), "a23f2a940a9cb8e5110b0e45");
    }

    #[test]
    fn distinct_full_digests_can_share_v3_path_suffix_without_adoption() {
        const FULL_A: &str = "a23f2a940a9cb8e5110b0e454ad68a35a22af760c005bcd0ebdf47ab44330270";
        const FULL_B: &str = "a23f2a940a9cb8e5110b0e45ffffffffffffffffffffffffffffffffffffffff";
        assert!(digests_share_v3_path_suffix(FULL_A, FULL_B));
        assert_ne!(FULL_A, FULL_B);
        assert_eq!(
            isolated_prefix_path_v3("fixture-server", FULL_A),
            isolated_prefix_path_v3("fixture-server", FULL_B)
        );
    }

    #[test]
    fn incompatible_manifest_schema_is_reported() {
        let root = test_prefix("schema");
        create_structure(&root);
        let manifest = serde_json::json!({
            "schemaVersion": PREFIX_SCHEMA_V3 + 1,
            "scope": "shared",
            "serverId": null,
            "runnerKind": "wine",
            "runnerPath": "/usr/bin/wine",
            "components": []
        });
        std::fs::write(
            root.join(PREFIX_MARKER),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let health = inspect_prefix(root.to_str().unwrap());
        assert!(health
            .issues
            .iter()
            .any(|issue| issue.contains("schema incompatible")));
        std::fs::remove_dir_all(root).unwrap();
    }
}
