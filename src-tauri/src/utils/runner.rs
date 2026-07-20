use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::models::server::ServerConfig;
use crate::tools::runners::{managed_proton_path, managed_umu_path};
use crate::utils::prefix::effective_prefix;
use crate::utils::{
    apply_prefix_env, ensure_custom_setup_allowed, ensure_managed_path_safe, find_umu_run,
    inspect_prefix, is_executable_file, manifest_matches_location, manifest_matches_runner,
    proton_vkd3d_companions_available, resolve_server_prefix, winetricks_path, PrefixHealth,
    PrefixLocation, PrefixScope, PREFIX_SCHEMA_VERSION, UMU_RUN_BIN,
};

const DEFAULT_GAME_ID: &str = "0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    Wine,
    Proton,
}

#[derive(Debug, Clone)]
enum RunnerStrategy {
    Wine {
        wine_bin: PathBuf,
        wineserver_bin: PathBuf,
    },
    Proton {
        proton_script: PathBuf,
        proton_dir: PathBuf,
        umu_bin: PathBuf,
    },
}

/// Runner resuelto como una estrategia completa.
#[derive(Debug, Clone)]
pub struct ResolvedRunner {
    strategy: RunnerStrategy,
}

impl ResolvedRunner {
    pub fn kind(&self) -> RunnerKind {
        match self.strategy {
            RunnerStrategy::Wine { .. } => RunnerKind::Wine,
            RunnerStrategy::Proton { .. } => RunnerKind::Proton,
        }
    }

    pub fn is_proton(&self) -> bool {
        self.kind() == RunnerKind::Proton
    }

    pub fn runner_path(&self) -> &Path {
        match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => wine_bin,
            RunnerStrategy::Proton { proton_script, .. } => proton_script,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind() {
            RunnerKind::Wine => "wine",
            RunnerKind::Proton => "proton",
        }
    }

    pub fn proton_root(&self) -> Option<&Path> {
        match &self.strategy {
            RunnerStrategy::Proton { proton_dir, .. } => Some(proton_dir),
            RunnerStrategy::Wine { .. } => None,
        }
    }

    pub fn supports_winetricks_verb(&self, verb: &str) -> bool {
        let script = match &self.strategy {
            RunnerStrategy::Proton { proton_dir, .. } => {
                Some(proton_dir.join("protonfixes/winetricks"))
            }
            RunnerStrategy::Wine { .. } => winetricks_path(),
        };
        script
            .and_then(|script| std::fs::read_to_string(script).ok())
            .is_some_and(|content| {
                content.lines().any(|line| {
                    let mut words = line.split_whitespace();
                    words.next() == Some("w_metadata") && words.next() == Some(verb)
                })
            })
    }

    pub fn game_command<I, S>(
        &self,
        prefix_path: &str,
        exe_path: &str,
        args: I,
        work_dir: &str,
    ) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let mut cmd = Command::new(wine_bin);
                cmd.arg(exe_path).args(args);
                self.apply_wine_env(&mut cmd, prefix_path);
                cmd
            }
            RunnerStrategy::Proton { .. } => {
                let mut cmd = self.proton_command(prefix_path, ProtonVerb::WaitForExitAndRun);
                cmd.arg(exe_path).args(args);
                cmd
            }
        };
        cmd.current_dir(work_dir);
        cmd
    }

    pub fn tool_command<I, S>(
        &self,
        prefix_path: &str,
        exe_path: &str,
        args: I,
        work_dir: &str,
    ) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let mut cmd = Command::new(wine_bin);
                cmd.arg(exe_path).args(args);
                self.apply_wine_env(&mut cmd, prefix_path);
                cmd
            }
            RunnerStrategy::Proton { .. } => {
                let mut cmd = self.proton_command(prefix_path, ProtonVerb::Run);
                cmd.arg(exe_path).args(args);
                cmd
            }
        };
        cmd.current_dir(work_dir);
        cmd
    }

    /// Ejecuta un programa incorporado de Wine (`reg`, `msiexec`, `wineboot`, etc.).
    pub fn builtin_command<I, S>(&self, prefix_path: &str, program: &str, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let mut cmd = Command::new(wine_bin);
                cmd.arg(program).args(args);
                self.apply_wine_env(&mut cmd, prefix_path);
                cmd
            }
            RunnerStrategy::Proton { .. } => {
                let mut cmd = self.proton_command(prefix_path, ProtonVerb::RunInPrefix);
                cmd.arg(program).args(args);
                cmd
            }
        }
    }

    pub fn create_prefix_command(&self, prefix_path: &str) -> Command {
        match &self.strategy {
            RunnerStrategy::Wine { .. } => self.builtin_command(prefix_path, "wineboot", ["-i"]),
            RunnerStrategy::Proton { .. } => {
                let mut cmd = self.proton_command(prefix_path, ProtonVerb::WaitForExitAndRun);
                // UMU documenta un argumento vacío como la operación para crear el prefix.
                cmd.arg("");
                cmd
            }
        }
    }

    pub fn winetricks_command<I, S>(&self, prefix_path: &str, packages: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        match &self.strategy {
            RunnerStrategy::Wine {
                wine_bin,
                wineserver_bin,
            } => {
                let mut cmd =
                    Command::new(winetricks_path().unwrap_or_else(|| PathBuf::from("winetricks")));
                cmd.arg("-q").args(packages);
                self.apply_wine_env(&mut cmd, prefix_path);
                cmd.env("WINE", wine_bin).env("WINESERVER", wineserver_bin);
                cmd
            }
            RunnerStrategy::Proton { .. } => {
                let mut cmd = self.proton_command(prefix_path, ProtonVerb::WaitForExitAndRun);
                // UMU selecciona el winetricks incluido en la distribución Proton y agrega -q.
                cmd.arg("winetricks").args(packages);
                cmd
            }
        }
    }

    pub fn shutdown_command(&self, prefix_path: &str) -> Command {
        match &self.strategy {
            RunnerStrategy::Wine { wineserver_bin, .. } => {
                let mut cmd = Command::new(wineserver_bin);
                cmd.arg("-k");
                self.apply_wine_env(&mut cmd, prefix_path);
                cmd
            }
            RunnerStrategy::Proton { .. } => self.builtin_command(prefix_path, "wineboot", ["-k"]),
        }
    }

    fn apply_wine_env(&self, cmd: &mut Command, prefix_path: &str) {
        apply_prefix_env(cmd, prefix_path);

        let RunnerStrategy::Wine {
            wine_bin,
            wineserver_bin,
        } = &self.strategy
        else {
            return;
        };

        cmd.env("WINE", wine_bin).env("WINESERVER", wineserver_bin);
        if let Some(bin_dir) = wine_bin.parent() {
            prepend_path(cmd, bin_dir);
        }
    }

    fn proton_command(&self, prefix_path: &str, verb: ProtonVerb) -> Command {
        let RunnerStrategy::Proton {
            proton_dir,
            umu_bin,
            ..
        } = &self.strategy
        else {
            unreachable!("proton_command sólo se usa con runners Proton");
        };

        let mut cmd = Command::new(umu_bin);
        apply_prefix_env(&mut cmd, prefix_path);
        cmd.env("PROTONPATH", proton_dir)
            .env("GAMEID", DEFAULT_GAME_ID)
            .env("PROTON_VERB", verb.as_str());
        cmd
    }
}

#[derive(Debug, Clone, Copy)]
enum ProtonVerb {
    Run,
    WaitForExitAndRun,
    RunInPrefix,
}

impl ProtonVerb {
    fn as_str(self) -> &'static str {
        match self {
            ProtonVerb::Run => "run",
            ProtonVerb::WaitForExitAndRun => "waitforexitandrun",
            ProtonVerb::RunInPrefix => "runinprefix",
        }
    }
}

/// WINEPREFIX + runner resueltos para un servidor (o defaults globales).
pub struct WineContext {
    pub prefix: String,
    pub location: PrefixLocation,
    pub resolved: ResolvedRunner,
}

pub fn runtime_prefix_blockers(ctx: &WineContext, health: &PrefixHealth) -> Vec<String> {
    let mut blockers = Vec::new();
    if let Err(error) = ensure_managed_path_safe(&ctx.location) {
        blockers.push(error);
    }
    if let Err(error) = ensure_custom_setup_allowed(&ctx.location) {
        blockers.push(error);
    }
    let structurally_ready = if ctx.location.scope == PrefixScope::Custom {
        health.structure_ok
    } else {
        health.configured
    };
    if !structurally_ready {
        blockers.extend(health.issues.clone());
    }
    if health.legacy_marker {
        blockers.push(
            "El entorno legacy no registra qué runner lo creó; debe rearmarse antes de lanzar"
                .to_string(),
        );
    }

    if let Some(manifest) = &health.manifest {
        if manifest.schema_version != PREFIX_SCHEMA_VERSION {
            blockers.push(format!(
                "Schema de entorno incompatible: {} (esperado {PREFIX_SCHEMA_VERSION})",
                manifest.schema_version
            ));
        }
        if !manifest_matches_location(manifest, &ctx.location) {
            blockers.push("El manifiesto pertenece a otro entorno o servidor".to_string());
        }
        if manifest.runner_kind == "unknown" {
            blockers.push(
                "El manifiesto no registra qué runner creó el entorno; debe rearmarse".to_string(),
            );
        } else if !manifest_matches_runner(
            manifest,
            ctx.resolved.kind_label(),
            ctx.resolved.runner_path().to_string_lossy().as_ref(),
        ) {
            blockers.push(format!(
                "El entorno fue creado con otro runner ({})",
                manifest.runner_path
            ));
        }
    }

    if let Some(root) = ctx.resolved.proton_root() {
        if !proton_vkd3d_companions_available(&ctx.prefix, root) {
            blockers
                .push("Proton no expone las DLL compañeras libvkd3d para x86 y x86_64".to_string());
        }
    }

    blockers.sort();
    blockers.dedup();
    blockers
}

pub fn validate_runtime_prefix(ctx: &WineContext) -> Result<PrefixHealth, String> {
    let health = inspect_prefix(&ctx.prefix);
    let blockers = runtime_prefix_blockers(ctx, &health);
    if blockers.is_empty() {
        Ok(health)
    } else {
        Err(format!(
            "El entorno {} no es compatible: {}",
            ctx.prefix,
            blockers.join(" · ")
        ))
    }
}

pub async fn resolve_wine_context(
    wine_prefix: Option<String>,
    _runner: Option<String>,
) -> Result<WineContext, String> {
    let prefix = effective_prefix(wine_prefix.clone());
    Ok(WineContext {
        location: PrefixLocation {
            path: prefix.clone(),
            scope: if wine_prefix.is_some() {
                crate::utils::PrefixScope::Custom
            } else {
                crate::utils::PrefixScope::Shared
            },
            managed: wine_prefix.is_none(),
            server_id: None,
        },
        prefix,
        resolved: resolve_effective_runner(None).await?,
    })
}

pub async fn resolve_server_wine_context_with_runner(
    server: Option<&ServerConfig>,
    _default_runner: Option<String>,
) -> Result<WineContext, String> {
    let location = resolve_server_prefix(server)?;
    let resolved = resolve_effective_runner(None).await?;
    Ok(WineContext {
        prefix: location.path.clone(),
        location,
        resolved,
    })
}

pub async fn resolve_effective_runner(
    _override_path: Option<String>,
) -> Result<ResolvedRunner, String> {
    let proton = managed_proton_path();
    let umu = managed_umu_path();
    resolve_runner_with_umu(proton.to_string_lossy().as_ref(), &umu)
}

pub fn resolve_runner(runner_path: &str) -> Result<ResolvedRunner, String> {
    let managed_umu = managed_umu_path();
    let umu = is_executable_file(&managed_umu)
        .then_some(managed_umu)
        .or_else(find_umu_run)
        .unwrap_or_else(|| PathBuf::from(UMU_RUN_BIN));
    resolve_runner_with_umu(runner_path, &umu)
}

fn resolve_runner_with_umu(runner_path: &str, umu_bin: &Path) -> Result<ResolvedRunner, String> {
    let path = Path::new(runner_path);
    if !is_executable_file(path) {
        return Err(format!("Runner no encontrado: {runner_path}"));
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("proton") {
        let proton_dir = path
            .parent()
            .ok_or_else(|| format!("Ruta Proton inválida: {runner_path}"))?;
        let proton_wine = proton_dir.join("files/bin/wine");
        if !is_executable_file(&proton_wine) {
            return Err(format!(
                "Distribución Proton incompleta: falta {}",
                proton_wine.display()
            ));
        }
        if !is_executable_file(umu_bin) {
            return Err(
                "Proton requiere umu-launcher; no se encontró umu-run en PATH ni en las rutas del sistema"
                    .to_string(),
            );
        }

        return Ok(ResolvedRunner {
            strategy: RunnerStrategy::Proton {
                proton_script: path.to_path_buf(),
                proton_dir: proton_dir.to_path_buf(),
                umu_bin: umu_bin.to_path_buf(),
            },
        });
    }

    let wineserver_bin = find_companion_wineserver(path)
        .ok_or_else(|| format!("No se encontró un wineserver compatible junto a {runner_path}"))?;

    Ok(ResolvedRunner {
        strategy: RunnerStrategy::Wine {
            wine_bin: path.to_path_buf(),
            wineserver_bin,
        },
    })
}

fn find_companion_wineserver(wine_bin: &Path) -> Option<PathBuf> {
    let parent = wine_bin.parent()?;
    let name = wine_bin.file_name()?.to_str()?;

    let mut candidates = Vec::new();
    if let Some(suffix) = name.strip_prefix("wine") {
        if !suffix.is_empty() {
            candidates.push(parent.join(format!("wineserver{suffix}")));
        }
    }
    candidates.push(parent.join("wineserver"));

    candidates
        .into_iter()
        .find(|candidate| is_executable_file(candidate))
}

fn prepend_path(cmd: &mut Command, bin_dir: &Path) {
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(path) = std::env::join_paths(paths) {
        cmd.env("PATH", path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_wine_runner() -> ResolvedRunner {
        ResolvedRunner {
            strategy: RunnerStrategy::Wine {
                wine_bin: PathBuf::from("/opt/test-wine/bin/wine"),
                wineserver_bin: PathBuf::from("/opt/test-wine/bin/wineserver"),
            },
        }
    }

    fn test_proton_runner() -> ResolvedRunner {
        ResolvedRunner {
            strategy: RunnerStrategy::Proton {
                proton_script: PathBuf::from("/opt/test-proton/proton"),
                proton_dir: PathBuf::from("/opt/test-proton"),
                umu_bin: PathBuf::from(UMU_RUN_BIN),
            },
        }
    }

    fn args(command: &Command) -> Vec<OsString> {
        command
            .as_std()
            .get_args()
            .map(OsStr::to_os_string)
            .collect()
    }

    fn env(command: &Command, key: &str) -> Option<OsString> {
        command
            .as_std()
            .get_envs()
            .find(|(name, _)| *name == OsStr::new(key))
            .and_then(|(_, value)| value.map(OsStr::to_os_string))
    }

    #[test]
    fn wine_game_uses_matching_binary_and_preserves_arguments() {
        let runner = test_wine_runner();
        let command = runner.game_command(
            "/tmp/prefix",
            "/games/My RO/ragexe.exe",
            ["-1rag1", "value with spaces"],
            "/games/My RO",
        );

        assert_eq!(
            command.as_std().get_program(),
            OsStr::new("/opt/test-wine/bin/wine")
        );
        assert_eq!(
            args(&command),
            ["/games/My RO/ragexe.exe", "-1rag1", "value with spaces"].map(OsString::from)
        );
        assert_eq!(env(&command, "WINEPREFIX"), Some("/tmp/prefix".into()));
        assert_eq!(
            env(&command, "WINESERVER"),
            Some("/opt/test-wine/bin/wineserver".into())
        );
    }

    #[test]
    fn proton_game_uses_umu_and_never_inner_wine() {
        let runner = test_proton_runner();
        let command = runner.game_command(
            "/tmp/proton-prefix",
            "/games/RO/ragexe.exe",
            ["account", "secret with spaces"],
            "/games/RO",
        );

        assert_eq!(command.as_std().get_program(), OsStr::new(UMU_RUN_BIN));
        assert_eq!(
            args(&command),
            ["/games/RO/ragexe.exe", "account", "secret with spaces"].map(OsString::from)
        );
        assert_eq!(env(&command, "PROTONPATH"), Some("/opt/test-proton".into()));
        assert_eq!(env(&command, "GAMEID"), Some(DEFAULT_GAME_ID.into()));
        assert_eq!(
            env(&command, "PROTON_VERB"),
            Some("waitforexitandrun".into())
        );
    }

    #[test]
    fn proton_operations_select_the_expected_verbs() {
        let runner = test_proton_runner();
        let tool =
            runner.tool_command("/tmp/p", "/games/setup.exe", Vec::<String>::new(), "/games");
        let builtin = runner.builtin_command("/tmp/p", "reg", ["query", "HKCU\\Software"]);
        let create = runner.create_prefix_command("/tmp/p");

        assert_eq!(env(&tool, "PROTON_VERB"), Some("run".into()));
        assert_eq!(env(&builtin, "PROTON_VERB"), Some("runinprefix".into()));
        assert_eq!(args(&create), [OsString::from("")]);
        assert_eq!(
            env(&create, "PROTON_VERB"),
            Some("waitforexitandrun".into())
        );
    }

    #[test]
    fn proton_winetricks_does_not_inject_dxvk() {
        let runner = test_proton_runner();
        let command = runner.winetricks_command("/tmp/p", ["vcrun2019", "d3dx9"]);

        assert_eq!(
            args(&command),
            ["winetricks", "vcrun2019", "d3dx9"].map(OsString::from)
        );
        assert!(!args(&command).iter().any(|arg| arg == "dxvk"));
    }

    #[test]
    fn wine_shutdown_uses_matching_wineserver() {
        let command = test_wine_runner().shutdown_command("/tmp/p");
        assert_eq!(
            command.as_std().get_program(),
            OsStr::new("/opt/test-wine/bin/wineserver")
        );
        assert_eq!(args(&command), [OsString::from("-k")]);
    }

    #[test]
    fn legacy_prefix_without_runner_identity_is_blocked_until_rearm() {
        let context = WineContext {
            prefix: "/tmp/legacy-prefix".to_string(),
            location: PrefixLocation {
                path: "/tmp/legacy-prefix".to_string(),
                scope: PrefixScope::Shared,
                managed: true,
                server_id: None,
            },
            resolved: test_wine_runner(),
        };
        let health = PrefixHealth {
            structure_ok: true,
            configured: true,
            legacy_marker: true,
            manifest: None,
            issues: vec!["Entorno legacy".to_string()],
        };
        assert!(runtime_prefix_blockers(&context, &health)
            .iter()
            .any(|issue| issue.contains("debe rearmarse")));
    }

    #[test]
    fn proton_checks_only_its_effective_winetricks_script() {
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-proton-winetricks-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("protonfixes")).unwrap();
        std::fs::write(
            root.join("protonfixes/winetricks"),
            "w_metadata webview2 dlls\n",
        )
        .unwrap();
        let runner = ResolvedRunner {
            strategy: RunnerStrategy::Proton {
                proton_script: root.join("proton"),
                proton_dir: root.clone(),
                umu_bin: PathBuf::from(UMU_RUN_BIN),
            },
        };

        assert!(runner.supports_winetricks_verb("webview2"));
        assert!(!runner.supports_winetricks_verb("corefonts"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
