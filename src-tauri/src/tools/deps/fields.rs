use std::path::Path;

const WINE_HINT: &str = "Instala Wine: sudo pacman -S wine-cachyos";
const WINETRICKS_HINT: &str = "Instala winetricks: sudo pacman -S winetricks";
const PREFIX_SETUP_HINT: &str = "WINEPREFIX no configurado. Pulsa Configurar en Jugar.";
const PREFIX_INCOMPLETE_HINT: &str =
    "Setup del prefix incompleto. Pulsa Configurar en Jugar.";
const DXVK_PENDING_HINT: &str = "Disponible tras configurar el prefix.";
const DXVK_MISSING_HINT: &str =
    "DXVK no encontrado en el prefix. Vuelve a configurar (winetricks dxvk).";

pub fn dependency_prefix_fields(
    wine: bool,
    winetricks: bool,
    prefix_configured: bool,
    prefix_path: &str,
) -> (bool, Option<String>) {
    if !wine {
        return (false, Some(WINE_HINT.to_string()));
    }
    if !winetricks {
        return (false, Some(WINETRICKS_HINT.to_string()));
    }
    if prefix_configured {
        return (true, None);
    }

    let partial = prefix_dir_started(prefix_path);
    let hint = if partial {
        PREFIX_INCOMPLETE_HINT
    } else {
        PREFIX_SETUP_HINT
    };
    (false, Some(hint.to_string()))
}

pub fn dependency_dxvk_fields(dxvk: bool, prefix_configured: bool) -> (bool, Option<String>) {
    if dxvk {
        return (true, None);
    }
    if !prefix_configured {
        return (true, Some(DXVK_PENDING_HINT.to_string()));
    }
    (false, Some(DXVK_MISSING_HINT.to_string()))
}

fn prefix_dir_started(prefix_path: &str) -> bool {
    let path = Path::new(prefix_path);
    if !path.is_dir() {
        return false;
    }
    path.read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_requires_wine_and_winetricks() {
        assert_eq!(
            dependency_prefix_fields(false, true, false, "/tmp/p"),
            (false, Some(WINE_HINT.to_string()))
        );
        assert_eq!(
            dependency_prefix_fields(true, false, false, "/tmp/p"),
            (false, Some(WINETRICKS_HINT.to_string()))
        );
    }

    #[test]
    fn prefix_ok_when_configured() {
        assert_eq!(
            dependency_prefix_fields(true, true, true, "/tmp/p"),
            (true, None)
        );
    }

    #[test]
    fn dxvk_pending_without_prefix() {
        assert_eq!(
            dependency_dxvk_fields(false, false),
            (true, Some(DXVK_PENDING_HINT.to_string()))
        );
    }

    #[test]
    fn dxvk_error_when_prefix_ready_but_missing() {
        assert_eq!(
            dependency_dxvk_fields(false, true),
            (false, Some(DXVK_MISSING_HINT.to_string()))
        );
    }
}
