use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tokio::process::Command;

pub trait ProcessEnv {
    fn set_env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>);
    #[allow(dead_code)]
    fn unset_env(&mut self, key: impl AsRef<OsStr>);
}

impl ProcessEnv for Command {
    fn set_env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) {
        self.env(key, val);
    }

    fn unset_env(&mut self, key: impl AsRef<OsStr>) {
        self.env_remove(key);
    }
}

pub fn apply_prefix_env<E: ProcessEnv>(env: &mut E, prefix_path: &str) {
    env.set_env("WINEPREFIX", prefix_path);
    env.set_env("WAYLAND_DISPLAY", "");
}

/// Variante para `Command` directo: sanea AppImage además del prefix.
#[allow(dead_code)]
pub fn apply_prefix_env_command(cmd: &mut Command, prefix_path: &str) {
    sanitize_appimage_env(cmd);
    apply_prefix_env(cmd, prefix_path);
}

pub fn apply_game_env<E: ProcessEnv>(
    env: &mut E,
    use_dgvoodoo: bool,
    use_managed_dxvk: bool,
    prefix_path: &str,
) {
    env.set_env("WINE_LARGE_ADDRESS_AWARE", "1");

    if use_managed_dxvk {
        let overrides = if use_dgvoodoo {
            "d3dimm=n,b;ddraw=n,b;d3d8=n,b;d3d9=n,b;d3d10core=n,b;d3d11=n,b;dxgi=n,b"
        } else {
            "d3d8=n,b;d3d9=n,b;d3d10core=n,b;d3d11=n,b;dxgi=n,b"
        };
        env.set_env("WINEDLLOVERRIDES", overrides);
        env.set_env("DXVK_CONFIG_FILE", dxvk_config_path(prefix_path));
        env.set_env("DXVK_LOG_PATH", dxvk_log_path(prefix_path));
        env.set_env("DXVK_STATE_CACHE_PATH", dxvk_cache_path(prefix_path));
    } else if use_dgvoodoo {
        env.set_env("WINEDLLOVERRIDES", "d3dimm=n,b;ddraw=n,b");
    }
}

pub fn dxvk_config_path(prefix_path: &str) -> PathBuf {
    dxvk_state_root(prefix_path).join("dxvk.conf")
}

pub fn dxvk_log_path(prefix_path: &str) -> PathBuf {
    dxvk_state_root(prefix_path).join("logs")
}

pub fn dxvk_cache_path(prefix_path: &str) -> PathBuf {
    dxvk_state_root(prefix_path).join("cache")
}

pub fn dxvk_state_root(prefix_path: &str) -> PathBuf {
    Path::new(prefix_path).join(".ro-launcher-dxvk")
}

/// OpenSetup y el patcher deben enumerar la misma GPU y backend que usará el juego.
pub fn apply_tool_env<E: ProcessEnv>(
    env: &mut E,
    use_dgvoodoo: bool,
    use_managed_dxvk: bool,
    prefix_path: &str,
) {
    apply_game_env(env, use_dgvoodoo, use_managed_dxvk, prefix_path);
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
pub(crate) fn sanitize_appimage_env(cmd: &mut Command) {
    let Some(app_dir) = std::env::var_os("APPDIR") else {
        return;
    };
    sanitize_appimage_env_with(cmd, Path::new(&app_dir), |key| std::env::var_os(key));
}

/// Returns the host-facing PATH that external runners should receive.
///
/// Supervised invocations serialize PATH after `ro-sessiond` was spawned, so they cannot rely on
/// command-level sanitation alone: an explicit delta would overwrite the sidecar's clean PATH.
pub(crate) fn sanitized_external_path() -> Option<OsString> {
    sanitized_external_path_from(std::env::var_os("PATH"), std::env::var_os("APPDIR"))
}

fn sanitized_external_path_from(
    path: Option<OsString>,
    app_dir: Option<OsString>,
) -> Option<OsString> {
    let path = path?;
    let Some(app_dir) = app_dir else {
        return Some(path);
    };
    Some(sanitize_path_value(&path, Path::new(&app_dir)))
}

fn sanitize_appimage_env_with<F>(cmd: &mut Command, app_dir: &Path, mut current_var: F)
where
    F: FnMut(&str) -> Option<OsString>,
{
    for key in APPIMAGE_PATH_ENV {
        let Some(value) = current_var(key) else {
            continue;
        };
        if *key == "PATH" {
            cmd.env(key, sanitize_path_value(&value, app_dir));
            continue;
        }
        match filter_appimage_paths(&value, app_dir) {
            Some(filtered) => {
                cmd.env(key, filtered);
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

fn sanitize_path_value(value: &OsStr, app_dir: &Path) -> OsString {
    filter_appimage_paths(value, app_dir)
        .unwrap_or_else(|| OsString::from("/usr/local/bin:/usr/bin:/bin"))
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
        assert_eq!(
            sanitized_external_path_from(
                Some("/tmp/.mount_RO/usr/bin:/usr/bin".into()),
                Some("/tmp/.mount_RO".into())
            ),
            Some("/usr/bin".into())
        );
    }

    #[test]
    fn managed_dxvk_uses_native_graphics_dlls_and_a_real_config_file() {
        let mut command = Command::new("/usr/bin/true");
        apply_game_env(&mut command, true, true, "/tmp/prefix");

        assert_eq!(
            command_env(&command, "DXVK_CONFIG_FILE"),
            Some(Some("/tmp/prefix/.ro-launcher-dxvk/dxvk.conf".into()))
        );
        assert_eq!(
            command_env(&command, "WINEDLLOVERRIDES"),
            Some(Some(
                "d3dimm=n,b;ddraw=n,b;d3d8=n,b;d3d9=n,b;d3d10core=n,b;d3d11=n,b;dxgi=n,b".into()
            ))
        );
        assert_eq!(command_env(&command, "DXVK_HUD"), None);
        assert_eq!(command_env(&command, "DXVK_ASYNC"), None);
        assert_eq!(command_env(&command, "PROTON_USE_WOW64"), None);
    }

    #[test]
    fn legacy_target_matrix_keeps_maintenance_patcher_without_dgvoodoo() {
        let overlay_and_dxvk = OsString::from(
            "d3dimm=n,b;ddraw=n,b;d3d8=n,b;d3d9=n,b;d3d10core=n,b;d3d11=n,b;dxgi=n,b",
        );
        let dxvk_only = OsString::from("d3d8=n,b;d3d9=n,b;d3d10core=n,b;d3d11=n,b;dxgi=n,b");
        for (target, use_dgvoodoo, expected) in [
            ("game", true, overlay_and_dxvk.clone()),
            ("launch-patcher", true, overlay_and_dxvk.clone()),
            ("maintenance-patcher", false, dxvk_only.clone()),
            ("open-setup", true, overlay_and_dxvk.clone()),
            ("graphics-control-panel", true, overlay_and_dxvk.clone()),
        ] {
            let mut command = Command::new("/usr/bin/true");
            apply_game_env(&mut command, use_dgvoodoo, true, "/tmp/prefix");
            assert_eq!(
                command_env(&command, "WINEDLLOVERRIDES"),
                Some(Some(expected)),
                "target {target}"
            );
            assert_eq!(
                command_env(&command, "WINE_LARGE_ADDRESS_AWARE"),
                Some(Some("1".into())),
                "target {target}"
            );
        }
    }
}
