use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::models::server::ServerConfig;
use crate::tools::runners::{managed_proton_path, managed_umu_path};
use crate::tools::runtime::{probe_runner, resolve_prefix_binding, PrefixBinding, RunnerProbe};
use crate::utils::{
    apply_prefix_env, ensure_custom_setup_allowed, ensure_managed_path_safe, find_umu_run,
    inspect_prefix, is_executable_file, manifest_matches_runner, manifest_matches_stored_location,
    proton_vkd3d_companions_available, sanitize_appimage_env,
    sanitized_external_path, winetricks_path, PrefixHealth, PrefixLocation, PrefixScope,
    ProcessEnv, PREFIX_SCHEMA_V3, PREFIX_SCHEMA_VERSION, UMU_RUN_BIN,
};

const DEFAULT_GAME_ID: &str = "0";

#[derive(Debug, Clone)]
pub struct RunnerInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, Option<OsString>)>,
}

impl ProcessEnv for RunnerInvocation {
    fn set_env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) {
        let key = key.as_ref().to_os_string();
        self.env.retain(|(existing, _)| existing != &key);
        self.env.push((key, Some(val.as_ref().to_os_string())));
    }

    fn unset_env(&mut self, key: impl AsRef<OsStr>) {
        let key = key.as_ref().to_os_string();
        self.env.retain(|(existing, _)| existing != &key);
        self.env.push((key, None));
    }
}

impl RunnerInvocation {
    pub fn into_command(self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(self.args).current_dir(self.cwd);
        cmd.env_remove("WINEPREFIX");
        cmd.env_remove("STEAM_COMPAT_DATA_PATH");
        for (key, value) in self.env {
            match value {
                Some(val) => {
                    cmd.env(key, val);
                }
                None => {
                    cmd.env_remove(key);
                }
            }
        }
        sanitize_appimage_env(&mut cmd);
        cmd
    }
}

fn absolute_program(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "El programa del runner debe ser una ruta absoluta: {}",
            path.display()
        ));
    }
    Ok(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn invocation_cwd(work_dir: &str) -> PathBuf {
    let path = Path::new(work_dir);
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn invocation_cwd_for_prefix(prefix_path: &str) -> PathBuf {
    let path = Path::new(prefix_path);
    if path.is_dir() {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    } else if let Ok(cwd) = std::env::current_dir() {
        std::fs::canonicalize(&cwd).unwrap_or(cwd)
    } else {
        path.to_path_buf()
    }
}

fn resolve_winetricks_executable() -> Result<PathBuf, String> {
    let path = effective_wine_winetricks_path()
        .or(winetricks_path())
        .ok_or_else(|| {
            "winetricks no encontrado en PATH ni en el runtime administrado; instálalo en el sistema"
                .to_string()
        })?;
    if path.is_absolute() {
        return absolute_program(&path);
    }
    which_in_path(&path)
}

fn which_in_path(name: &Path) -> Result<PathBuf, String> {
    let file_name = name
        .file_name()
        .ok_or_else(|| format!("Ruta inválida: {}", name.display()))?;
    let path_var = sanitized_external_path().unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(file_name);
        if is_executable_file(&candidate) {
            return absolute_program(&candidate);
        }
    }
    Err(format!(
        "No se encontró {} en PATH",
        file_name.to_string_lossy()
    ))
}

fn prepend_path_env<E: ProcessEnv>(env: &mut E, bin_dir: &Path) {
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(existing) = sanitized_external_path() {
        paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(path) = std::env::join_paths(paths) {
        env.set_env("PATH", path);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerKind {
    Wine,
    Proton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WineSyncMode {
    WineServer,
    Esync,
    Fsync,
}

impl WineSyncMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::WineServer => "wineserver",
            Self::Esync => "esync",
            Self::Fsync => "fsync/futex_waitv",
        }
    }
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

pub(crate) fn resolved_managed_proton_descriptor() -> ResolvedRunner {
    let proton_script = managed_proton_path();
    let proton_dir = proton_script
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| proton_script.clone());
    ResolvedRunner {
        strategy: RunnerStrategy::Proton {
            proton_script,
            proton_dir,
            umu_bin: managed_umu_path(),
        },
    }
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

    pub fn reported_version(&self) -> Option<String> {
        match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let mut command = Command::new(wine_bin);
                sanitize_appimage_env(&mut command);
                command.arg("--version");
                command
                    .as_std_mut()
                    .output()
                    .ok()
                    .filter(|output| output.status.success())
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .filter(|version| !version.is_empty())
            }
            RunnerStrategy::Proton { proton_dir, .. } => {
                std::fs::read_to_string(proton_dir.join("version"))
                    .ok()
                    .and_then(|version| {
                        version
                            .lines()
                            .next()
                            .map(str::trim)
                            .filter(|version| !version.is_empty())
                            .map(str::to_owned)
                    })
            }
        }
    }

    pub fn is_wine_7_16(&self) -> bool {
        self.kind() == RunnerKind::Wine
            && self
                .reported_version()
                .is_some_and(|version| is_wine_7_16_version(&version))
    }

    /// Activa únicamente mecanismos que el propio artefacto declara haber incorporado.
    /// Un Wine vanilla conserva wineserver; nunca se le inyectan flags que no implementa.
    pub fn wine_sync_mode(&self) -> WineSyncMode {
        let RunnerStrategy::Wine { wine_bin, .. } = &self.strategy else {
            return WineSyncMode::WineServer;
        };
        let config = wine_bin
            .parent()
            .and_then(Path::parent)
            .map(|root| root.join("wine-tkg-config.txt"))
            .and_then(|path| std::fs::read_to_string(path).ok());
        sync_mode_from_tkg_config(config.as_deref())
    }

    pub fn proton_root(&self) -> Option<&Path> {
        match &self.strategy {
            RunnerStrategy::Proton { proton_dir, .. } => Some(proton_dir),
            RunnerStrategy::Wine { .. } => None,
        }
    }

    pub(crate) fn wineserver_path(&self) -> Option<&Path> {
        match &self.strategy {
            RunnerStrategy::Wine { wineserver_bin, .. } => Some(wineserver_bin),
            RunnerStrategy::Proton { .. } => None,
        }
    }

    pub(crate) fn proton_umu_path(&self) -> Option<&Path> {
        match &self.strategy {
            RunnerStrategy::Proton { umu_bin, .. } => Some(umu_bin),
            RunnerStrategy::Wine { .. } => None,
        }
    }

    pub fn supports_winetricks_verb(&self, verb: &str) -> bool {
        self.winetricks_script()
            .and_then(|script| std::fs::read_to_string(script).ok())
            .is_some_and(|content| {
                content.lines().any(|line| {
                    let mut words = line.split_whitespace();
                    words.next() == Some("w_metadata") && words.next() == Some(verb)
                })
            })
    }

    pub fn has_winetricks(&self) -> bool {
        self.winetricks_script()
            .is_some_and(|path| is_executable_file(&path))
    }

    fn winetricks_script(&self) -> Option<PathBuf> {
        match &self.strategy {
            RunnerStrategy::Proton { proton_dir, .. } => {
                Some(proton_dir.join("protonfixes/winetricks"))
            }
            RunnerStrategy::Wine { .. } => effective_wine_winetricks_path(),
        }
    }

    pub fn game_invocation<I, S>(
        &self,
        prefix_path: &str,
        exe_path: &str,
        args: I,
        work_dir: &str,
    ) -> Result<RunnerInvocation, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut invocation = match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let program = absolute_program(wine_bin)?;
                let mut args_vec = vec![OsString::from(exe_path)];
                args_vec.extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
                RunnerInvocation {
                    program,
                    args: args_vec,
                    cwd: invocation_cwd(work_dir),
                    env: Vec::new(),
                }
            }
            RunnerStrategy::Proton { .. } => {
                let mut invocation =
                    self.proton_invocation(prefix_path, ProtonVerb::WaitForExitAndRun)?;
                invocation.args.push(OsString::from(exe_path));
                invocation
                    .args
                    .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
                invocation.cwd = invocation_cwd(work_dir);
                invocation
            }
        };
        self.apply_wine_env(&mut invocation, prefix_path)?;
        Ok(invocation)
    }

    #[allow(dead_code)] // fase 4 y tests; producción usa *_invocation.
    #[cfg(test)]
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
        self.game_invocation(prefix_path, exe_path, args, work_dir)
            .expect("game_invocation")
            .into_command()
    }

    pub fn tool_invocation<I, S>(
        &self,
        prefix_path: &str,
        exe_path: &str,
        args: I,
        work_dir: &str,
    ) -> Result<RunnerInvocation, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut invocation = match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let program = absolute_program(wine_bin)?;
                let mut args_vec = vec![OsString::from(exe_path)];
                args_vec.extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
                RunnerInvocation {
                    program,
                    args: args_vec,
                    cwd: invocation_cwd(work_dir),
                    env: Vec::new(),
                }
            }
            RunnerStrategy::Proton { .. } => {
                let mut invocation = self.proton_invocation(prefix_path, ProtonVerb::Run)?;
                invocation.args.push(OsString::from(exe_path));
                invocation
                    .args
                    .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
                invocation.cwd = invocation_cwd(work_dir);
                invocation
            }
        };
        self.apply_wine_env(&mut invocation, prefix_path)?;
        Ok(invocation)
    }

    #[cfg(test)]
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
        self.tool_invocation(prefix_path, exe_path, args, work_dir)
            .expect("tool_invocation")
            .into_command()
    }

    /// Ejecuta un programa incorporado de Wine (`reg`, `msiexec`, `wineboot`, etc.).
    pub fn builtin_invocation<I, S>(
        &self,
        prefix_path: &str,
        program: &str,
        args: I,
    ) -> Result<RunnerInvocation, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut invocation = match &self.strategy {
            RunnerStrategy::Wine { wine_bin, .. } => {
                let program_path = absolute_program(wine_bin)?;
                RunnerInvocation {
                    program: program_path,
                    args: std::iter::once(OsString::from(program))
                        .chain(args.into_iter().map(|a| a.as_ref().to_os_string()))
                        .collect(),
                    cwd: invocation_cwd_for_prefix(prefix_path),
                    env: Vec::new(),
                }
            }
            RunnerStrategy::Proton { .. } => {
                let mut invocation =
                    self.proton_invocation(prefix_path, ProtonVerb::RunInPrefix)?;
                invocation.args.push(OsString::from(program));
                invocation
                    .args
                    .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
                invocation.cwd = invocation_cwd_for_prefix(prefix_path);
                invocation
            }
        };
        self.apply_wine_env(&mut invocation, prefix_path)?;
        Ok(invocation)
    }

    #[cfg(test)]
    pub fn builtin_command<I, S>(&self, prefix_path: &str, program: &str, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.builtin_invocation(prefix_path, program, args)
            .expect("builtin_invocation")
            .into_command()
    }

    pub fn create_prefix_invocation(&self, prefix_path: &str) -> Result<RunnerInvocation, String> {
        match &self.strategy {
            RunnerStrategy::Wine { .. } => self.builtin_invocation(prefix_path, "wineboot", ["-i"]),
            RunnerStrategy::Proton { .. } => {
                let mut invocation =
                    self.proton_invocation(prefix_path, ProtonVerb::WaitForExitAndRun)?;
                invocation.args.push(OsString::new());
                Ok(invocation)
            }
        }
    }

    #[cfg(test)]
    pub fn create_prefix_command(&self, prefix_path: &str) -> Command {
        self.create_prefix_invocation(prefix_path)
            .expect("create_prefix_invocation")
            .into_command()
    }

    pub fn winetricks_invocation<I, S>(
        &self,
        prefix_path: &str,
        packages: I,
    ) -> Result<RunnerInvocation, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        match &self.strategy {
            RunnerStrategy::Wine {
                wine_bin,
                wineserver_bin,
            } => {
                let program = resolve_winetricks_executable()?;
                let mut invocation = RunnerInvocation {
                    program,
                    args: std::iter::once(OsString::from("-q"))
                        .chain(packages.into_iter().map(|p| p.as_ref().to_os_string()))
                        .collect(),
                    cwd: invocation_cwd_for_prefix(prefix_path),
                    env: Vec::new(),
                };
                self.apply_wine_env(&mut invocation, prefix_path)?;
                invocation.set_env("WINE", wine_bin.as_os_str());
                invocation.set_env("WINESERVER", wineserver_bin.as_os_str());
                Ok(invocation)
            }
            RunnerStrategy::Proton { .. } => {
                let mut invocation =
                    self.proton_invocation(prefix_path, ProtonVerb::WaitForExitAndRun)?;
                invocation.args.push(OsString::from("winetricks"));
                invocation
                    .args
                    .extend(packages.into_iter().map(|p| p.as_ref().to_os_string()));
                Ok(invocation)
            }
        }
    }

    #[cfg(test)]
    pub fn winetricks_command<I, S>(&self, prefix_path: &str, packages: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.winetricks_invocation(prefix_path, packages)
            .expect("winetricks_invocation")
            .into_command()
    }

    pub fn shutdown_invocation(&self, prefix_path: &str) -> Result<RunnerInvocation, String> {
        match &self.strategy {
            RunnerStrategy::Wine { wineserver_bin, .. } => {
                let program = absolute_program(wineserver_bin)?;
                let mut invocation = RunnerInvocation {
                    program,
                    args: vec![OsString::from("-k")],
                    cwd: invocation_cwd_for_prefix(prefix_path),
                    env: Vec::new(),
                };
                self.apply_wine_env(&mut invocation, prefix_path)?;
                Ok(invocation)
            }
            RunnerStrategy::Proton { .. } => {
                self.builtin_invocation(prefix_path, "wineboot", ["-k"])
            }
        }
    }

    #[cfg(test)]
    pub fn shutdown_command(&self, prefix_path: &str) -> Command {
        self.shutdown_invocation(prefix_path)
            .expect("shutdown_invocation")
            .into_command()
    }

    fn apply_wine_env(
        &self,
        invocation: &mut RunnerInvocation,
        prefix_path: &str,
    ) -> Result<(), String> {
        apply_prefix_env(invocation, prefix_path);

        let RunnerStrategy::Wine {
            wine_bin,
            wineserver_bin,
        } = &self.strategy
        else {
            return Ok(());
        };

        invocation.set_env("WINE", wine_bin.as_os_str());
        invocation.set_env("WINESERVER", wineserver_bin.as_os_str());
        match self.wine_sync_mode() {
            WineSyncMode::WineServer => {}
            WineSyncMode::Esync => {
                invocation.set_env("WINEESYNC", "1");
            }
            WineSyncMode::Fsync => {
                invocation.set_env("WINEFSYNC", "1");
            }
        }
        if let Some(bin_dir) = wine_bin.parent() {
            prepend_path_env(invocation, bin_dir);
        }
        Ok(())
    }

    fn proton_invocation(
        &self,
        prefix_path: &str,
        verb: ProtonVerb,
    ) -> Result<RunnerInvocation, String> {
        let RunnerStrategy::Proton {
            proton_dir,
            umu_bin,
            ..
        } = &self.strategy
        else {
            unreachable!("proton_invocation sólo se usa con runners Proton");
        };

        let program = absolute_program(umu_bin)?;
        let mut invocation = RunnerInvocation {
            program,
            args: Vec::new(),
            cwd: invocation_cwd_for_prefix(prefix_path),
            env: Vec::new(),
        };
        apply_prefix_env(&mut invocation, prefix_path);
        invocation.set_env("PROTONPATH", proton_dir.as_os_str());
        invocation.set_env("GAMEID", DEFAULT_GAME_ID);
        invocation.set_env("PROTON_VERB", verb.as_str());
        Ok(invocation)
    }
}

pub(crate) fn is_wine_7_16_version(version: &str) -> bool {
    version.strip_prefix("wine-").is_some_and(|version| {
        version == "7.16"
            || version.starts_with("7.16 ")
            || version.starts_with("7.16.")
            || version.starts_with("7.16-")
    })
}

#[cfg(test)]
impl ResolvedRunner {
    pub(crate) fn test_wine(wine_bin: PathBuf, wineserver_bin: PathBuf) -> Self {
        Self {
            strategy: RunnerStrategy::Wine {
                wine_bin,
                wineserver_bin,
            },
        }
    }

    pub(crate) fn test_proton(
        proton_script: PathBuf,
        proton_dir: PathBuf,
        umu_bin: PathBuf,
    ) -> Self {
        Self {
            strategy: RunnerStrategy::Proton {
                proton_script,
                proton_dir,
                umu_bin,
            },
        }
    }
}

fn sync_mode_from_tkg_config(config: Option<&str>) -> WineSyncMode {
    let Some(config) = config else {
        return WineSyncMode::WineServer;
    };
    if config.contains("fsync-unix-staging.patch") && config.contains("fsync_futex_waitv.patch") {
        WineSyncMode::Fsync
    } else if config.contains("Using wine-staging patchset") {
        WineSyncMode::Esync
    } else {
        WineSyncMode::WineServer
    }
}

fn effective_wine_winetricks_path() -> Option<PathBuf> {
    preferred_wine_winetricks_path(&managed_proton_path(), winetricks_path())
}

fn preferred_wine_winetricks_path(
    managed_proton: &Path,
    fallback: Option<PathBuf>,
) -> Option<PathBuf> {
    let bundled = managed_proton
        .parent()
        .map(|root| root.join("protonfixes/winetricks"));
    bundled.filter(|path| is_executable_file(path)).or(fallback)
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
#[derive(Debug, Clone)]
pub struct WineContext {
    pub prefix: String,
    pub location: PrefixLocation,
    pub resolved: ResolvedRunner,
    pub identity: PrefixBinding,
    #[allow(dead_code)]
    pub probe: RunnerProbe,
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
        let schema = manifest.schema_version();
        if schema != PREFIX_SCHEMA_VERSION && schema != PREFIX_SCHEMA_V3 {
            blockers.push(format!(
                "Schema de entorno incompatible: {} (esperado {PREFIX_SCHEMA_VERSION} o {PREFIX_SCHEMA_V3})",
                schema
            ));
        }
        if !manifest_matches_stored_location(manifest, &ctx.location) {
            blockers.push("El manifiesto pertenece a otro entorno o servidor".to_string());
        }
        if manifest.runner_kind() == "unknown" {
            blockers.push(
                "El manifiesto no registra qué runner creó el entorno; debe rearmarse".to_string(),
            );
        } else if let Some(v2) = manifest.as_v2() {
            if !manifest_matches_runner(
                v2,
                ctx.resolved.kind_label(),
                ctx.resolved.runner_path().to_string_lossy().as_ref(),
            ) {
                blockers.push(format!(
                    "El entorno fue creado con otro runner ({})",
                    manifest.runner_path()
                ));
            }
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
    _wine_prefix: Option<String>,
    runner: Option<String>,
) -> Result<WineContext, String> {
    let resolved = resolve_effective_runner(runner).await?;
    let probe = probe_runner(&resolved);
    let identity = resolve_prefix_binding(None, &resolved, &probe, resolved.is_wine_7_16())?;
    let location = identity.location.clone();
    let prefix = location.path.clone();
    Ok(WineContext {
        location,
        prefix,
        resolved,
        identity,
        probe,
    })
}

pub async fn resolve_server_wine_context_with_runner(
    server: Option<&ServerConfig>,
    default_runner: Option<String>,
) -> Result<WineContext, String> {
    let selected_runner = select_effective_runner(server, default_runner);
    let resolved = resolve_effective_runner(selected_runner).await?;
    let probe = probe_runner(&resolved);
    let identity = resolve_prefix_binding(server, &resolved, &probe, resolved.is_wine_7_16())?;
    let location = identity.location.clone();
    Ok(WineContext {
        prefix: location.path.clone(),
        location,
        resolved,
        identity,
        probe,
    })
}

fn select_effective_runner(
    server: Option<&ServerConfig>,
    default_runner: Option<String>,
) -> Option<String> {
    server
        .and_then(|server| server.runner.as_ref())
        .filter(|runner| !runner.trim().is_empty())
        .cloned()
        .or(default_runner)
}

pub async fn resolve_effective_runner(
    override_path: Option<String>,
) -> Result<ResolvedRunner, String> {
    if let Some(path) = override_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return resolve_runner(path);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn invocation_env_replaces_duplicate_keys() {
        let mut invocation = RunnerInvocation {
            program: PathBuf::from("/usr/bin/true"),
            args: vec![],
            cwd: PathBuf::from("/tmp"),
            env: Vec::new(),
        };
        invocation.set_env("FOO", "1");
        invocation.set_env("FOO", "2");
        assert_eq!(invocation.env.len(), 1);
        assert_eq!(
            invocation.env[0]
                .1
                .as_ref()
                .map(|v| v.to_string_lossy().to_string()),
            Some("2".to_string())
        );
    }

    #[test]
    fn direct_invocation_clears_inherited_steam_prefix() {
        let invocation = RunnerInvocation {
            program: PathBuf::from("/usr/bin/true"),
            args: vec![],
            cwd: PathBuf::from("/tmp"),
            env: vec![(
                OsString::from("WINEPREFIX"),
                Some(OsString::from("/tmp/owned-prefix")),
            )],
        };
        let command = invocation.into_command();
        assert_eq!(
            env(&command, "WINEPREFIX"),
            Some("/tmp/owned-prefix".into())
        );
        assert!(command
            .as_std()
            .get_envs()
            .any(|(key, value)| key == OsStr::new("STEAM_COMPAT_DATA_PATH") && value.is_none()));
    }

    #[test]
    fn identifies_only_the_validated_wine_7_16_line() {
        assert!(is_wine_7_16_version("wine-7.16"));
        assert!(is_wine_7_16_version("wine-7.16 (Staging)"));
        assert!(is_wine_7_16_version(
            "wine-7.16.r0.gaa2eb6ee ( TkG Staging Esync Fsync )"
        ));
        assert!(!is_wine_7_16_version("wine-7.1"));
        assert!(!is_wine_7_16_version("wine-7.160"));
        assert!(!is_wine_7_16_version("wine-10.0"));
    }

    #[test]
    fn enables_only_the_sync_implementation_declared_by_tkg() {
        assert_eq!(sync_mode_from_tkg_config(None), WineSyncMode::WineServer);
        assert_eq!(
            sync_mode_from_tkg_config(Some("Using wine-staging patchset (version 7.16)")),
            WineSyncMode::Esync
        );
        assert_eq!(
            sync_mode_from_tkg_config(Some(
                "Using wine-staging patchset\nfsync-unix-staging.patch\nfsync_futex_waitv.patch"
            )),
            WineSyncMode::Fsync
        );
    }

    #[test]
    fn server_override_precedes_global_and_empty_override_does_not() {
        let mut server: ServerConfig = serde_json::from_value(serde_json::json!({
            "id": "server",
            "name": "Server",
            "executablePath": "/games/ro/Ragexe.exe"
        }))
        .unwrap();
        assert_eq!(
            select_effective_runner(Some(&server), Some("/global/proton".to_string())),
            Some("/global/proton".to_string())
        );

        server.runner = Some("/server/wine".to_string());
        assert_eq!(
            select_effective_runner(Some(&server), Some("/global/proton".to_string())),
            Some("/server/wine".to_string())
        );

        server.runner = Some("  ".to_string());
        assert_eq!(
            select_effective_runner(Some(&server), Some("/global/proton".to_string())),
            Some("/global/proton".to_string())
        );
        assert_eq!(select_effective_runner(Some(&server), None), None);
    }

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
    fn tkg_fsync_is_applied_to_every_wine_command() {
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-wine-fsync-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(
            root.join("wine-tkg-config.txt"),
            "Using wine-staging patchset\nfsync-unix-staging.patch\nfsync_futex_waitv.patch\n",
        )
        .unwrap();
        let runner = ResolvedRunner {
            strategy: RunnerStrategy::Wine {
                wine_bin: root.join("bin/wine"),
                wineserver_bin: root.join("bin/wineserver"),
            },
        };

        let game = runner.game_command("/tmp/p", "/games/ragexe.exe", ["-1rag1"], "/games");
        let shutdown = runner.shutdown_command("/tmp/p");
        assert_eq!(runner.wine_sync_mode(), WineSyncMode::Fsync);
        assert_eq!(env(&game, "WINEFSYNC"), Some("1".into()));
        assert_eq!(env(&game, "WINEESYNC"), None);
        assert_eq!(env(&shutdown, "WINEFSYNC"), Some("1".into()));

        std::fs::remove_dir_all(root).unwrap();
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
    fn wine_prefers_the_managed_winetricks_script() {
        let root = std::env::temp_dir().join(format!(
            "ro-launcher-wine-winetricks-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let script = root.join("protonfixes/winetricks");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, "#!/bin/sh\n").unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        assert_eq!(
            preferred_wine_winetricks_path(
                &root.join("proton"),
                Some(PathBuf::from("/usr/bin/winetricks")),
            ),
            Some(script)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_prefix_without_runner_identity_is_blocked_until_rearm() {
        let resolved = resolved_managed_proton_descriptor();
        let probe = probe_runner(&resolved);
        let identity = resolve_prefix_binding(None, &resolved, &probe, resolved.is_wine_7_16())
            .expect("test binding");
        let location = PrefixLocation {
            path: "/tmp/legacy-prefix".to_string(),
            scope: PrefixScope::Shared,
            managed: true,
            server_id: None,
        };
        let context = WineContext {
            prefix: location.path.clone(),
            location,
            resolved,
            identity,
            probe,
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
