use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

use crate::models::launch::LaunchStrategy;
use crate::models::server::ServerConfig;
use crate::models::server_tools::{ClientDiagnostics, ToolInfo};
use crate::utils::find_file_case_insensitive;

const MAX_PE_SIZE: u64 = 128 * 1024 * 1024;
const MAX_DEPENDENCIES: usize = 96;

#[derive(Debug, Clone, Default)]
struct PeInfo {
    architecture: Option<&'static str>,
    imports: Vec<String>,
    managed: bool,
}

pub fn inspect_client(
    server: &ServerConfig,
    game_dir: &Path,
    patcher: &ToolInfo,
) -> ClientDiagnostics {
    let game_exe = Path::new(&server.executable_path);
    let parsed_root = parse_pe(game_exe);
    let root = parsed_root.as_ref().cloned().unwrap_or_default();
    let imports = collect_local_imports(game_dir, &root);
    let mut graphics = Vec::new();
    if imports.contains("ddraw.dll") || imports.contains("d3dimm.dll") {
        graphics.push("DirectDraw / DirectX 1-7".to_string());
    }
    if imports.contains("d3d8.dll") {
        graphics.push("Direct3D 8".to_string());
    }
    if imports.contains("d3d9.dll") || imports.iter().any(|name| name.starts_with("d3dx9_")) {
        graphics.push("Direct3D 9".to_string());
    }
    if imports.contains("d3d11.dll") || imports.contains("dxgi.dll") {
        graphics.push("Direct3D 11".to_string());
    }

    let patcher_info = patcher
        .path
        .as_deref()
        .and_then(|path| parse_pe(Path::new(path)));
    let managed_patcher = patcher_info.as_ref().is_some_and(|info| info.managed);
    let patcher_requires_webview2 = patcher
        .path
        .as_deref()
        .is_some_and(|path| executable_requires_webview2(Path::new(path)));
    let launches_through_patcher = server.launch.strategy == LaunchStrategy::Patcher;
    let webview2_required = server.launch.require_webview2
        || imports.contains("webview2loader.dll")
        || (launches_through_patcher && patcher_requires_webview2);
    let pe_analysis_conclusive =
        parsed_root.is_some() && (!launches_through_patcher || patcher_info.is_some());
    let gepard_present = find_file_case_insensitive(game_dir, "gepard.dll").is_some();
    let gameguard_present = find_file_case_insensitive(game_dir, "gameguard.des").is_some();

    let mut warnings = Vec::new();
    if graphics.iter().any(|api| api == "Direct3D 9")
        && graphics.iter().any(|api| api == "DirectDraw / DirectX 1-7")
    {
        warnings.push(
            "El cliente carga DirectDraw y Direct3D 9; cambiar el modo gráfico no evita resolver ambas cadenas de DLL."
                .to_string(),
        );
    }
    if webview2_required {
        warnings.push(
            "Una dependencia local usa WebView2Loader; el loader no sustituye al Edge WebView2 Runtime."
                .to_string(),
        );
    }
    if patcher_requires_webview2 && !launches_through_patcher {
        warnings.push(
            "El patcher opcional usa WebView2, pero no bloquea el inicio directo. Para abrirlo, activa el requisito manual y repara el entorno si el runtime falta."
                .to_string(),
        );
    }
    if server.launch.require_webview2 {
        warnings.push(
            "WebView2 fue marcado como requisito manual para cubrir carga dinámica o PE protegido."
                .to_string(),
        );
    } else if !pe_analysis_conclusive {
        warnings.push(
            "El análisis PE no fue concluyente. Si el cliente o patcher usa una interfaz web, activa el requisito manual de WebView2."
                .to_string(),
        );
    }
    if managed_patcher {
        warnings.push(
            "El patcher es administrado (.NET); Wine Mono puede renderizar distinto a .NET Framework nativo."
                .to_string(),
        );
    }
    if gepard_present || gameguard_present {
        warnings.push(
            "Se detectó anti-cheat. Confirma con el servidor si Wine, DXVK y la versión de dgVoodoo están permitidos."
                .to_string(),
        );
    }

    ClientDiagnostics {
        architecture: root.architecture.map(str::to_string),
        graphics_apis: graphics,
        managed_patcher,
        webview2_required,
        pe_analysis_conclusive,
        gepard_present,
        gameguard_present,
        warnings,
    }
}

pub fn requires_webview2(server: &ServerConfig) -> bool {
    if server.launch.require_webview2 {
        return true;
    }
    if executable_requires_webview2(Path::new(&server.executable_path)) {
        return true;
    }
    server.launch.strategy == LaunchStrategy::Patcher
        && detected_patcher_path(server)
            .as_deref()
            .is_some_and(|path| executable_requires_webview2(Path::new(path)))
}

pub fn webview2_runtime_present(prefix: &Path) -> bool {
    [
        prefix.join("drive_c/Program Files (x86)/Microsoft/EdgeUpdate/MicrosoftEdgeUpdate.exe"),
        prefix.join("drive_c/Program Files/Microsoft/EdgeUpdate/MicrosoftEdgeUpdate.exe"),
    ]
    .iter()
    .any(|path| path.is_file() && !path.is_symlink())
}

pub fn missing_runtime_components(server: &ServerConfig, prefix: &Path) -> Vec<String> {
    if requires_webview2(server) && !webview2_runtime_present(prefix) {
        vec![
            "El cliente requiere Microsoft Edge WebView2 Runtime y no está instalado en este entorno"
                .to_string(),
        ]
    } else {
        Vec::new()
    }
}

pub fn missing_runtime_components_for_executable(
    executable: &Path,
    prefix: &Path,
    manual_webview2: bool,
) -> Vec<String> {
    if (manual_webview2 || executable_requires_webview2(executable))
        && !webview2_runtime_present(prefix)
    {
        vec![
            "La herramienta requiere Microsoft Edge WebView2 Runtime y no está instalado en este entorno; activa «Forzar WebView2» y repara el entorno antes de abrirla"
                .to_string(),
        ]
    } else {
        Vec::new()
    }
}

fn executable_requires_webview2(executable: &Path) -> bool {
    let Some(game_dir) = executable.parent() else {
        return false;
    };
    let Some(root) = parse_pe(executable) else {
        return false;
    };
    collect_local_imports(game_dir, &root).contains("webview2loader.dll")
}

fn detected_patcher_path(server: &ServerConfig) -> Option<String> {
    if let Some(path) = server
        .patcher_path
        .as_ref()
        .filter(|path| Path::new(path).is_file())
    {
        return Some(path.clone());
    }
    let game_dir = Path::new(&server.executable_path).parent()?;
    super::scan::detect_patcher(game_dir, server).path
}

fn collect_local_imports(game_dir: &Path, root: &PeInfo) -> BTreeSet<String> {
    let mut imports = BTreeSet::new();
    let mut parsed = BTreeSet::new();
    let mut queue = VecDeque::from(root.imports.clone());

    while let Some(name) = queue.pop_front() {
        let name = name.to_ascii_lowercase();
        if !imports.insert(name.clone()) || parsed.len() >= MAX_DEPENDENCIES {
            continue;
        }
        let Some(local) = find_file_case_insensitive(game_dir, &name) else {
            continue;
        };
        if !parsed.insert(local.clone()) {
            continue;
        }
        if let Some(info) = parse_pe(&local) {
            queue.extend(info.imports);
        }
    }
    imports
}

fn parse_pe(path: &Path) -> Option<PeInfo> {
    if path.metadata().ok()?.len() > MAX_PE_SIZE {
        return None;
    }
    parse_pe_bytes(&std::fs::read(path).ok()?)
}

fn parse_pe_bytes(bytes: &[u8]) -> Option<PeInfo> {
    if bytes.get(0..2)? != b"MZ" {
        return None;
    }
    let pe = read_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }

    let coff = pe.checked_add(4)?;
    let machine = read_u16(bytes, coff)?;
    let section_count = read_u16(bytes, coff + 2)? as usize;
    let optional_size = read_u16(bytes, coff + 16)? as usize;
    let optional = coff.checked_add(20)?;
    let magic = read_u16(bytes, optional)?;
    let (data_directory, fallback_arch, image_base) = match magic {
        0x10b => (
            optional.checked_add(96)?,
            Some("x86"),
            u64::from(read_u32(bytes, optional + 28)?),
        ),
        0x20b => (
            optional.checked_add(112)?,
            Some("x86_64"),
            read_u64(bytes, optional + 24)?,
        ),
        _ => return None,
    };
    let architecture = match machine {
        0x14c => Some("x86"),
        0x8664 => Some("x86_64"),
        _ => fallback_arch,
    };

    let import_rva = read_u32(bytes, data_directory + 8)?;
    let delay_import_rva = read_u32(bytes, data_directory + (13 * 8)).unwrap_or(0);
    let clr_rva = read_u32(bytes, data_directory + (14 * 8)).unwrap_or(0);
    let sections_offset = optional.checked_add(optional_size)?;
    let sections = parse_sections(bytes, sections_offset, section_count)?;
    let mut imports = Vec::new();

    if import_rva != 0 {
        let mut descriptor = rva_to_offset(import_rva, &sections)?;
        for _ in 0..MAX_DEPENDENCIES {
            let entry = bytes.get(descriptor..descriptor.checked_add(20)?)?;
            if entry.iter().all(|byte| *byte == 0) {
                break;
            }
            let name_rva = u32::from_le_bytes(entry[12..16].try_into().ok()?);
            if let Some(name_offset) = rva_to_offset(name_rva, &sections) {
                if let Some(name) = read_c_string(bytes, name_offset) {
                    imports.push(name.to_ascii_lowercase());
                }
            }
            descriptor = descriptor.checked_add(20)?;
        }
    }

    if delay_import_rva != 0 {
        let mut descriptor = rva_to_offset(delay_import_rva, &sections)?;
        for _ in 0..MAX_DEPENDENCIES {
            let entry = bytes.get(descriptor..descriptor.checked_add(32)?)?;
            if entry.iter().all(|byte| *byte == 0) {
                break;
            }
            let attributes = u32::from_le_bytes(entry[0..4].try_into().ok()?);
            let raw_name = u32::from_le_bytes(entry[4..8].try_into().ok()?);
            let name_rva = if attributes & 1 != 0 {
                Some(raw_name)
            } else {
                u64::from(raw_name)
                    .checked_sub(image_base)
                    .and_then(|value| u32::try_from(value).ok())
            };
            if let Some(name_offset) = name_rva.and_then(|rva| rva_to_offset(rva, &sections)) {
                if let Some(name) = read_c_string(bytes, name_offset) {
                    imports.push(name.to_ascii_lowercase());
                }
            }
            descriptor = descriptor.checked_add(32)?;
        }
    }

    Some(PeInfo {
        architecture,
        imports,
        managed: clr_rva != 0,
    })
}

#[derive(Debug)]
struct Section {
    virtual_address: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

fn parse_sections(bytes: &[u8], offset: usize, count: usize) -> Option<Vec<Section>> {
    if count > 96 {
        return None;
    }
    let mut sections = Vec::with_capacity(count);
    for index in 0..count {
        let section = offset.checked_add(index.checked_mul(40)?)?;
        sections.push(Section {
            virtual_size: read_u32(bytes, section + 8)?,
            virtual_address: read_u32(bytes, section + 12)?,
            raw_size: read_u32(bytes, section + 16)?,
            raw_offset: read_u32(bytes, section + 20)?,
        });
    }
    Some(sections)
}

fn rva_to_offset(rva: u32, sections: &[Section]) -> Option<usize> {
    for section in sections {
        let size = section.virtual_size.max(section.raw_size);
        let end = section.virtual_address.checked_add(size)?;
        if (section.virtual_address..end).contains(&rva) {
            let relative = rva.checked_sub(section.virtual_address)?;
            if relative >= section.raw_size {
                return None;
            }
            return section
                .raw_offset
                .checked_add(relative)
                .map(|value| value as usize);
        }
    }
    Some(rva as usize)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn read_c_string(bytes: &[u8], offset: usize) -> Option<String> {
    let tail = bytes.get(offset..)?;
    let end = tail.iter().position(|byte| *byte == 0)?;
    if end == 0 || end > 260 {
        return None;
    }
    Some(String::from_utf8_lossy(&tail[..end]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn minimal_pe_with_import(name: &str, delayed: bool) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x500];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        bytes[coff..coff + 2].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        bytes[coff + 16..coff + 18].copy_from_slice(&0xe0u16.to_le_bytes());
        let optional = coff + 20;
        bytes[optional..optional + 2].copy_from_slice(&0x10bu16.to_le_bytes());
        bytes[optional + 28..optional + 32].copy_from_slice(&0x400000u32.to_le_bytes());
        let data_directory = optional + 96;
        let descriptor_rva = 0x1000u32;
        let directory_index = if delayed { 13 } else { 1 };
        let directory = data_directory + directory_index * 8;
        bytes[directory..directory + 4].copy_from_slice(&descriptor_rva.to_le_bytes());

        let section = optional + 0xe0;
        bytes[section + 8..section + 12].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[section + 16..section + 20].copy_from_slice(&0x200u32.to_le_bytes());
        bytes[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());

        let name_rva = 0x1080u32;
        if delayed {
            bytes[0x200..0x204].copy_from_slice(&1u32.to_le_bytes());
            bytes[0x204..0x208].copy_from_slice(&name_rva.to_le_bytes());
        } else {
            bytes[0x20c..0x210].copy_from_slice(&name_rva.to_le_bytes());
        }
        let name_offset = 0x280;
        bytes[name_offset..name_offset + name.len()].copy_from_slice(name.as_bytes());
        bytes[name_offset + name.len()] = 0;
        bytes
    }

    fn test_server(game: &Path, patcher: Option<&Path>) -> ServerConfig {
        ServerConfig {
            id: "test".to_string(),
            name: "Test RO".to_string(),
            executable_path: game.to_string_lossy().to_string(),
            patcher_path: patcher.map(|path| path.to_string_lossy().to_string()),
            wine_prefix: None,
            prefix_mode: None,
            runner: None,
            launch: Default::default(),
            autopot: Default::default(),
            spammer: Default::default(),
            autobuff: Default::default(),
        }
    }

    #[test]
    fn rejects_non_pe_data() {
        assert!(parse_pe_bytes(b"not a PE").is_none());
    }

    #[test]
    fn c_string_is_bounded() {
        assert_eq!(
            read_c_string(b"ddraw.dll\0tail", 0).as_deref(),
            Some("ddraw.dll")
        );
        assert!(read_c_string(b"\0", 0).is_none());
    }

    #[test]
    fn parses_normal_and_delay_imports() {
        let normal = parse_pe_bytes(&minimal_pe_with_import("WebView2Loader.dll", false)).unwrap();
        assert!(normal
            .imports
            .iter()
            .any(|name| name == "webview2loader.dll"));

        let delayed = parse_pe_bytes(&minimal_pe_with_import("WebView2Loader.dll", true)).unwrap();
        assert!(delayed
            .imports
            .iter()
            .any(|name| name == "webview2loader.dll"));
    }

    #[test]
    fn webview2_requirement_follows_the_active_executable_and_manual_override() {
        let dir = std::env::temp_dir().join(format!(
            "ro-launcher-pe-patcher-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let game = dir.join("ragexe.exe");
        let patcher = dir.join("patcher.exe");
        std::fs::write(&game, minimal_pe_with_import("kernel32.dll", false)).unwrap();
        std::fs::write(&patcher, minimal_pe_with_import("WebView2Loader.dll", true)).unwrap();

        let mut server = test_server(&game, Some(&patcher));
        assert!(!requires_webview2(&server));
        let diagnostics = inspect_client(
            &server,
            &dir,
            &ToolInfo {
                found: true,
                path: Some(patcher.to_string_lossy().to_string()),
                label: None,
            },
        );
        assert!(!diagnostics.webview2_required);
        assert!(diagnostics
            .warnings
            .iter()
            .any(|warning| warning.contains("no bloquea el inicio directo")));
        assert!(missing_runtime_components_for_executable(&game, &dir, false).is_empty());
        assert!(!missing_runtime_components_for_executable(&game, &dir, true).is_empty());
        server.launch.strategy = LaunchStrategy::Patcher;
        assert!(requires_webview2(&server));
        std::fs::remove_file(&patcher).unwrap();
        server.launch.strategy = LaunchStrategy::Direct;
        server.launch.require_webview2 = true;
        assert!(requires_webview2(&server));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
