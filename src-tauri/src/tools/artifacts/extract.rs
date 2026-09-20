use std::fs::File;
use std::io::BufReader;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use tar::{Archive, EntryType};
use xz2::read::XzDecoder;

use super::descriptor::{ArchiveLayout, ArtifactDescriptor};
use super::payload::payload_ready;

pub(crate) fn extract_archive(
    descriptor: &ArtifactDescriptor,
    archive_path: &Path,
    staging_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir(staging_dir)
        .map_err(|error| format!("No se pudo crear staging: {error}"))?;
    let file = File::open(archive_path)
        .map_err(|error| format!("No se pudo abrir {}: {error}", archive_path.display()))?;

    match descriptor.archive {
        ArchiveLayout::Tar { .. } => {
            extract_entries(Archive::new(BufReader::new(file)), staging_dir, descriptor)?;
        }
        ArchiveLayout::TarGz { .. } => {
            let decoder = GzDecoder::new(BufReader::new(file));
            extract_entries(Archive::new(decoder), staging_dir, descriptor)?;
        }
        ArchiveLayout::TarXz { .. } => {
            let decoder = XzDecoder::new(BufReader::new(file));
            extract_entries(Archive::new(decoder), staging_dir, descriptor)?;
        }
    }

    let extracted = staging_dir.join(descriptor.archive.archive_root());
    if !extracted.is_dir()
        || extracted
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!(
            "El contenido extraído de {} está incompleto o no es válido",
            descriptor.id
        ));
    }
    if !payload_ready(descriptor, &extracted) {
        return Err(format!(
            "El contenido extraído de {} está incompleto o no es válido",
            descriptor.id
        ));
    }
    Ok(())
}

fn extract_entries(
    mut archive: Archive<impl Read>,
    staging_dir: &Path,
    descriptor: &ArtifactDescriptor,
) -> Result<(), String> {
    for entry in archive
        .entries()
        .map_err(|error| format!("No se pudo leer {}: {error}", descriptor.id))?
    {
        let mut entry =
            entry.map_err(|error| format!("Entrada inválida en {}: {error}", descriptor.id))?;
        let entry_path = entry
            .path()
            .map_err(|error| format!("Ruta inválida en {}: {error}", descriptor.id))?
            .into_owned();
        if !path_is_valid_utf8(&entry_path) {
            return Err(format!("Ruta no UTF-8 en el archive de {}", descriptor.id));
        }
        let normalized = normalize_relative_path(&entry_path)?;
        validate_entry_type(&entry, staging_dir, &normalized)?;
        entry
            .unpack_in(staging_dir)
            .map_err(|error| format!("No se pudo extraer {}: {error}", descriptor.id))?;
    }
    Ok(())
}

use std::io::Read;

fn path_is_valid_utf8(path: &Path) -> bool {
    path.to_str().is_some()
}

pub(crate) fn normalize_relative_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Err("ruta absoluta en archive".to_string());
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("traversal fuera de staging".to_string());
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("componente de ruta inválido".to_string());
            }
        }
    }
    Ok(out)
}

fn validate_entry_type(
    entry: &tar::Entry<'_, impl Read>,
    staging_dir: &Path,
    normalized_path: &Path,
) -> Result<(), String> {
    let entry_type = entry.header().entry_type();
    match entry_type {
        EntryType::Symlink | EntryType::Link => {
            let link_name = entry
                .link_name()
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "symlink sin destino".to_string())?;
            if !path_is_valid_utf8(&link_name) {
                return Err("symlink con destino no UTF-8".to_string());
            }
            validate_link_target(
                staging_dir,
                normalized_path,
                &link_name,
                entry_type == EntryType::Link,
            )?;
        }
        EntryType::Regular
        | EntryType::Directory
        | EntryType::GNULongName
        | EntryType::GNULongLink => {}
        EntryType::Fifo | EntryType::Char | EntryType::Block | EntryType::Continuous => {
            return Err(format!(
                "tipo de entrada no permitido en archive: {:?}",
                entry_type
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_link_target(
    staging_dir: &Path,
    entry_path: &Path,
    link_name: &Path,
    archive_root_relative: bool,
) -> Result<(), String> {
    if link_name.is_absolute() {
        return Err("symlink absoluto en archive".to_string());
    }
    // A symlink target is relative to the directory containing the link, while
    // a tar hard-link target is relative to the archive root. Normalize only
    // after composing the symlink path so legitimate targets such as
    // `gamefixes-gog/foo.py -> ../gamefixes-steam/foo.py` are accepted without
    // permitting them to escape staging.
    let combined = if archive_root_relative {
        link_name.to_path_buf()
    } else {
        entry_path.parent().unwrap_or(Path::new("")).join(link_name)
    };
    let normalized = normalize_relative_path(&combined)?;
    let resolved = staging_dir.join(normalized);
    if !resolved.starts_with(staging_dir) {
        return Err("symlink fuera de staging".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::descriptor::{
        catalog_descriptor, ArchiveLayout, ArtifactDescriptor, ArtifactKind, ArtifactSource,
        ExpectedDigest, PayloadValidatorId,
    };
    use crate::tools::runtime::ArtifactArchitecture;
    use tar::Builder;

    fn test_descriptor(root: &'static str) -> ArtifactDescriptor {
        ArtifactDescriptor {
            id: "test-artifact",
            kind: ArtifactKind::Dxvk,
            version: "0",
            source: ArtifactSource::Https {
                url: "https://example.com/x.tar",
            },
            expected_size: 0,
            digest: ExpectedDigest::Sha256(
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            archive: ArchiveLayout::Tar {
                archive_name: "x.tar",
                root,
            },
            platform: "linux-x86_64",
            architectures: &[ArtifactArchitecture::X86_64],
            recipe_revision: 1,
            payload: PayloadValidatorId::DxvkPrefixDlls,
        }
    }

    fn write_tar(path: &Path, add: impl Fn(&mut Builder<&mut Vec<u8>>)) {
        let mut buffer = Vec::new();
        let mut builder = Builder::new(&mut buffer);
        add(&mut builder);
        builder.finish().unwrap();
        drop(builder);
        std::fs::write(path, buffer).unwrap();
    }

    #[test]
    fn extract_rejects_parent_dir_and_absolute_paths() {
        assert!(normalize_relative_path(Path::new("foo/../../etc/passwd")).is_err());
        assert!(normalize_relative_path(Path::new("/etc/passwd")).is_err());
    }

    #[test]
    fn extract_rejects_symlink_escaping_staging() {
        let archive =
            std::env::temp_dir().join(format!("ro-launcher-tar-symlink-{}", std::process::id()));
        write_tar(&archive, |builder| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_link_name("../../outside").unwrap();
            header.set_path("link").unwrap();
            header.set_size(0);
            header.set_cksum();
            builder.append(&header, &[] as &[u8]).unwrap();
        });
        let staging = archive.with_extension("staging");
        let descriptor = test_descriptor("ignored");
        assert!(extract_archive(&descriptor, &archive, &staging).is_err());
        let _ = std::fs::remove_file(archive);
        let _ = std::fs::remove_dir_all(staging);
    }

    #[test]
    fn extract_allows_relative_symlink_inside_staging() {
        let archive =
            std::env::temp_dir().join(format!("ro-launcher-tar-ok-symlink-{}", std::process::id()));
        let root = "umu";
        write_tar(&archive, |builder| {
            let mut header = tar::Header::new_gnu();
            header.set_size(1);
            header.set_entry_type(EntryType::Regular);
            header.set_path(format!("{root}/umu-run")).unwrap();
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, &b"x"[..]).unwrap();
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_link_name("umu-run").unwrap();
            header.set_path(format!("{root}/umu_run.py")).unwrap();
            header.set_size(0);
            header.set_cksum();
            builder.append(&header, &[] as &[u8]).unwrap();
        });
        let staging = archive.with_extension("staging");
        let descriptor = catalog_descriptor(crate::tools::artifacts::UMU_ID).unwrap();
        extract_archive(descriptor, &archive, &staging).unwrap();
        let _ = std::fs::remove_file(archive);
        let _ = std::fs::remove_dir_all(staging);
    }

    #[test]
    fn extract_allows_parent_symlink_that_stays_inside_staging() {
        let staging = std::env::temp_dir().join(format!(
            "ro-launcher-link-validation-{}",
            std::process::id()
        ));
        validate_link_target(
            &staging,
            Path::new("protonfixes/gamefixes-gog/fix.py"),
            Path::new("../gamefixes-steam/fix.py"),
            false,
        )
        .unwrap();
        assert!(validate_link_target(
            &staging,
            Path::new("protonfixes/fix.py"),
            Path::new("../../outside"),
            false,
        )
        .is_err());
    }

    #[test]
    fn extract_rejects_wrong_archive_root() {
        let archive =
            std::env::temp_dir().join(format!("ro-launcher-tar-wrong-root-{}", std::process::id()));
        write_tar(&archive, |builder| {
            let mut header = tar::Header::new_gnu();
            header.set_size(1);
            header.set_entry_type(EntryType::Regular);
            header.set_path("other-root/x").unwrap();
            header.set_cksum();
            builder.append(&header, &b"x"[..]).unwrap();
        });
        let staging = archive.with_extension("staging");
        let descriptor = catalog_descriptor(crate::tools::artifacts::UMU_ID).unwrap();
        assert!(extract_archive(descriptor, &archive, &staging).is_err());
        let _ = std::fs::remove_file(archive);
        let _ = std::fs::remove_dir_all(staging);
    }
}
