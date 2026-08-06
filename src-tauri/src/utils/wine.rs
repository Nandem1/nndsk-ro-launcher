use std::ffi::{OsStr, OsString};
use std::path::Path;

use tokio::process::Command;

pub fn apply_prefix_env(cmd: &mut Command, prefix_path: &str) {
    sanitize_appimage_env(cmd);
    cmd.env("WINEPREFIX", prefix_path)
        .env("WAYLAND_DISPLAY", "");
}

pub fn apply_game_env(cmd: &mut Command, use_dgvoodoo: bool) {
    cmd.env("DXVK_ASYNC", "1")
        .env("DXVK_CONFIG", "d3d9.forceSamplerTypeSpecConstants=True")
        .env("WINE_LARGE_ADDRESS_AWARE", "1");
    if use_dgvoodoo {
        cmd.env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b");
    }
}

pub fn apply_tool_env(cmd: &mut Command, needs_dgvoodoo_overrides: bool) {
    cmd.env("DXVK_ASYNC", "1")
        .env("WINE_LARGE_ADDRESS_AWARE", "1");
    if needs_dgvoodoo_overrides {
        cmd.env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b");
    }
}

pub fn pipe_output(cmd: &mut Command) {
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
}

const APPIMAGE_PATH_ENV: &[&str] = &[
    "PATH",
    "LD_LIBRARY_PATH",
    "PYTHONPATH",
    "XDG_DATA_DIRS",
    "PERLLIB",
    "QT_PLUGIN_PATH",
    "GSETTINGS_SCHEMA_DIR",
    "GTK_PATH",
    "GIO_EXTRA_MODULES",
    "GI_TYPELIB_PATH",
    "GST_PLUGIN_PATH",
    "GST_PLUGIN_PATH_1_0",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
];

const APPIMAGE_FILE_ENV: &[&str] = &[
    "PYTHONHOME",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_IM_MODULE_FILE",
    "GDK_PIXBUF_MODULE_FILE",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
];

const APPIMAGE_METADATA_ENV: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "PYTHONDONTWRITEBYTECODE",
];

/// Evita que Wine, Proton y UMU hereden rutas internas del AppImage.
///
/// linuxdeploy configura PYTHONHOME y varias rutas de librerías contra el montaje temporal del
/// AppImage. Esas variables son necesarias para la UI empaquetada, pero rompen procesos externos
/// como el `umu-run` administrado, cuyo Python debe usar el runtime del host.
fn sanitize_appimage_env(cmd: &mut Command) {
    let Some(app_dir) = std::env::var_os("APPDIR") else {
        return;
    };
    sanitize_appimage_env_with(cmd, Path::new(&app_dir), |key| std::env::var_os(key));
}

fn sanitize_appimage_env_with<F>(cmd: &mut Command, app_dir: &Path, mut current_var: F)
where
    F: FnMut(&str) -> Option<OsString>,
{
    for key in APPIMAGE_PATH_ENV {
        let Some(value) = current_var(key) else {
            continue;
        };
        match filter_appimage_paths(&value, app_dir) {
            Some(filtered) => {
                cmd.env(key, filtered);
            }
            None if *key == "PATH" => {
                cmd.env(key, "/usr/local/bin:/usr/bin:/bin");
            }
            None => {
                cmd.env_remove(key);
            }
        }
    }

    for key in APPIMAGE_FILE_ENV {
        if current_var(key).is_some_and(|value| {
            let path = Path::new(&value);
            path == app_dir || path.starts_with(app_dir)
        }) {
            cmd.env_remove(key);
        }
    }

    for key in APPIMAGE_METADATA_ENV {
        cmd.env_remove(key);
    }
}

fn filter_appimage_paths(value: &OsStr, app_dir: &Path) -> Option<OsString> {
    let paths = std::env::split_paths(value)
        .filter(|path| !path.as_os_str().is_empty())
        .filter(|path| path != app_dir && !path.starts_with(app_dir))
        .collect::<Vec<_>>();

    if paths.is_empty() {
        None
    } else {
        std::env::join_paths(paths).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn command_env(command: &Command, key: &str) -> Option<Option<OsString>> {
        command
            .as_std()
            .get_envs()
            .find(|(name, _)| *name == OsStr::new(key))
            .map(|(_, value)| value.map(OsStr::to_os_string))
    }

    #[test]
    fn external_runners_do_not_inherit_appimage_python_or_library_paths() {
        let app_dir = Path::new("/tmp/.mount_RO");
        let values = HashMap::from([
            (
                "PATH",
                OsString::from("/tmp/.mount_RO/usr/bin:/usr/local/bin:/usr/bin"),
            ),
            (
                "LD_LIBRARY_PATH",
                OsString::from("/tmp/.mount_RO/usr/lib:/opt/runner/lib:"),
            ),
            (
                "PYTHONPATH",
                OsString::from("/tmp/.mount_RO/usr/share/pyshared:"),
            ),
            ("PYTHONHOME", OsString::from("/tmp/.mount_RO/usr")),
            (
                "GDK_PIXBUF_MODULE_FILE",
                OsString::from("/tmp/.mount_RO/usr/lib/loaders.cache"),
            ),
        ]);
        let mut command = Command::new("/usr/bin/true");

        sanitize_appimage_env_with(&mut command, app_dir, |key| values.get(key).cloned());

        assert_eq!(
            command_env(&command, "PATH"),
            Some(Some("/usr/local/bin:/usr/bin".into()))
        );
        assert_eq!(
            command_env(&command, "LD_LIBRARY_PATH"),
            Some(Some("/opt/runner/lib".into()))
        );
        assert_eq!(command_env(&command, "PYTHONPATH"), Some(None));
        assert_eq!(command_env(&command, "PYTHONHOME"), Some(None));
        assert_eq!(command_env(&command, "GDK_PIXBUF_MODULE_FILE"), Some(None));
        assert_eq!(command_env(&command, "APPDIR"), Some(None));
        assert_eq!(command_env(&command, "APPIMAGE"), Some(None));
    }

    #[test]
    fn path_falls_back_to_host_binaries_when_appimage_owns_every_entry() {
        let app_dir = Path::new("/tmp/.mount_RO");
        let mut command = Command::new("/usr/bin/true");

        sanitize_appimage_env_with(&mut command, app_dir, |key| {
            (key == "PATH").then(|| OsString::from("/tmp/.mount_RO/usr/bin"))
        });

        assert_eq!(
            command_env(&command, "PATH"),
            Some(Some("/usr/local/bin:/usr/bin:/bin".into()))
        );
    }
}
