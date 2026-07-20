use std::fs;

#[derive(Debug, Clone)]
pub struct GameProcessCandidate {
    pub pid: u32,
    pub reason: String,
}

/// Identidad estable de un proceso Linux.
///
/// El PID puede reutilizarse después de que un proceso termina. `start_time` corresponde al campo
/// 22 de `/proc/<pid>/stat` (ticks desde el arranque) y permite distinguir ambas instancias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_time: u64,
}

/// Captura la identidad actual de `pid`, o `None` si el proceso ya no existe/no es accesible.
pub fn capture_process_identity(pid: u32) -> Option<ProcessIdentity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, start_time) = parse_proc_stat(&stat)?;
    Some(ProcessIdentity { pid, start_time })
}

/// Verifica que el PID siga apuntando a la misma instancia capturada.
pub fn verify_process_identity(identity: &ProcessIdentity) -> bool {
    capture_process_identity(identity.pid).as_ref() == Some(identity)
}

/// Devuelve procesos visibles que declaran exactamente este WINEPREFIX.
/// Se usa antes de mover/resetear un entorno para no adivinar qué wineserver debe detenerlo.
pub fn find_prefix_processes(wine_prefix: &str) -> Vec<ProcessIdentity> {
    let prefix_norm = normalize_prefix(wine_prefix);
    if prefix_norm.is_empty() {
        return Vec::new();
    }

    let Ok(proc_dir) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut identities = Vec::new();
    for entry in proc_dir.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(environment) = read_proc_nul_fields(pid, "environ") else {
            continue;
        };
        if prefix_matches(&environment, &prefix_norm) {
            if let Some(identity) = capture_process_identity(pid) {
                identities.push(identity);
            }
        }
    }
    identities.sort_by_key(|identity| identity.pid);
    identities
}

/// Resolve the PID of the RO client inside a Wine session.
pub fn resolve_game_pid(launcher_pid: u32, exe_path: &str, wine_prefix: &str) -> Option<u32> {
    find_game_processes(launcher_pid, exe_path, wine_prefix)
        .into_iter()
        .next()
        .map(|c| c.pid)
}

pub fn find_game_processes(
    launcher_pid: u32,
    exe_path: &str,
    wine_prefix: &str,
) -> Vec<GameProcessCandidate> {
    let exe_name = windows_basename(exe_path);
    let prefix_norm = normalize_prefix(wine_prefix);

    let mut candidates = Vec::new();

    if let Some(reason) = match_process(launcher_pid, exe_name, &prefix_norm, launcher_pid) {
        candidates.push(GameProcessCandidate {
            pid: launcher_pid,
            reason,
        });
    }

    if let Ok(proc_dir) = fs::read_dir("/proc") {
        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let Some(pid_str) = name.to_str() else {
                continue;
            };
            if !pid_str.bytes().all(|byte| byte.is_ascii_digit()) {
                continue;
            }
            let Ok(pid) = pid_str.parse::<u32>() else {
                continue;
            };
            if pid == launcher_pid {
                continue;
            }
            if let Some(reason) = match_process(pid, exe_name, &prefix_norm, launcher_pid) {
                candidates.push(GameProcessCandidate { pid, reason });
            }
        }
    }

    candidates.sort_by_key(|candidate| score_candidate(candidate, launcher_pid));
    candidates
}

fn score_candidate(candidate: &GameProcessCandidate, launcher_pid: u32) -> u32 {
    let mut score = 0;
    if candidate.pid == launcher_pid {
        score += 100;
    }
    if candidate.reason.contains("child") {
        score += 50;
    }
    if candidate.reason.contains("prefix") {
        score += 10;
    }
    if candidate.reason.contains("cmdline") {
        score += 5;
    }
    u32::MAX - score
}

fn match_process(pid: u32, exe_name: &str, prefix_norm: &str, launcher_pid: u32) -> Option<String> {
    let cmdline = read_proc_nul_fields(pid, "cmdline")?;
    if !process_matches_exe(pid, &cmdline, exe_name) {
        return None;
    }

    let environ = read_proc_nul_fields(pid, "environ").unwrap_or_default();
    let is_launcher = pid == launcher_pid;
    let is_child = !is_launcher && is_descendant_of(pid, launcher_pid);
    match_process_fields(&cmdline, &environ, prefix_norm, is_launcher, is_child)
}

fn match_process_fields(
    cmdline: &[Vec<u8>],
    environ: &[Vec<u8>],
    prefix_norm: &str,
    is_launcher: bool,
    is_child: bool,
) -> Option<String> {
    let prefix_matched = !prefix_norm.is_empty()
        && (prefix_matches(environ, prefix_norm) || cmdline_matches_prefix(cmdline, prefix_norm));

    // Con un prefix explícito, un exe homónimo de otra sesión no es candidato. Se conserva la
    // descendencia como respaldo para wrappers que limpian WINEPREFIX de sus procesos hijos.
    if !prefix_norm.is_empty() && !prefix_matched && !is_launcher && !is_child {
        return None;
    }

    let mut reasons = vec!["cmdline"];
    if prefix_matched {
        reasons.push("prefix");
    }
    if is_launcher {
        reasons.push("launcher");
    } else if is_child {
        reasons.push("child");
    }
    Some(reasons.join("+"))
}

fn process_matches_exe(pid: u32, cmdline: &[Vec<u8>], exe_name: &str) -> bool {
    let argv0_matches = cmdline.first().is_some_and(|arg| {
        let arg = String::from_utf8_lossy(arg);
        windows_basename(arg.trim_matches('"')).eq_ignore_ascii_case(exe_name)
    });
    if argv0_matches {
        return true;
    }

    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
    process_name_matches_exe(comm.trim(), exe_name)
}

fn process_name_matches_exe(process_name: &str, exe_name: &str) -> bool {
    if process_name.eq_ignore_ascii_case(exe_name) {
        return true;
    }

    // Linux limita comm a TASK_COMM_LEN - 1 bytes. Wine usa el nombre del proceso Windows,
    // por lo que una comparación truncada sigue distinguiendo wrappers como python/umu/srt.
    let truncated: String = exe_name.chars().take(15).collect();
    exe_name.chars().count() > 15 && process_name.eq_ignore_ascii_case(&truncated)
}

fn cmdline_matches_prefix(cmdline: &[Vec<u8>], prefix_norm: &str) -> bool {
    cmdline.iter().any(|arg| {
        let arg = String::from_utf8_lossy(arg);
        let candidate = arg
            .trim_matches('"')
            .split_once('=')
            .map(|(_, value)| value)
            .unwrap_or(arg.trim_matches('"'))
            .trim_end_matches('/');
        candidate == prefix_norm
            || candidate
                .strip_prefix(prefix_norm)
                .is_some_and(|suffix| suffix.starts_with('/'))
    })
}

fn prefix_matches(environ: &[Vec<u8>], prefix_norm: &str) -> bool {
    if prefix_norm.is_empty() {
        return true;
    }

    environ.iter().any(|entry| {
        let Some((key, value)) = split_once_byte(entry, b'=') else {
            return false;
        };
        if key != b"WINEPREFIX" {
            return false;
        }
        normalize_prefix(&String::from_utf8_lossy(value)) == prefix_norm
    })
}

fn split_once_byte(bytes: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == separator)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

fn windows_basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

pub fn normalize_prefix(prefix: &str) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    fs::canonicalize(prefix)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| prefix.to_string())
}

fn is_descendant_of(pid: u32, ancestor: u32) -> bool {
    if pid == ancestor {
        return true;
    }
    let mut current = pid;
    for _ in 0..64 {
        let Ok(ppid) = read_ppid(current) else {
            return false;
        };
        if ppid == ancestor {
            return true;
        }
        if ppid <= 1 || ppid == current {
            return false;
        }
        current = ppid;
    }
    false
}

fn read_ppid(pid: u32) -> Result<u32, ()> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| ())?;
    parse_proc_stat(&stat).map(|(ppid, _)| ppid).ok_or(())
}

fn parse_proc_stat(stat: &str) -> Option<(u32, u64)> {
    // `comm` está entre paréntesis y puede contener espacios o `)`, por eso se usa el último `)`.
    let close_paren = stat.rfind(')')?;
    let fields: Vec<&str> = stat.get(close_paren + 1..)?.split_whitespace().collect();
    // Tras comm: índice 0 = state (campo 3), 1 = ppid (campo 4), 19 = starttime (campo 22).
    let ppid = fields.get(1)?.parse().ok()?;
    let start_time = fields.get(19)?.parse().ok()?;
    Some((ppid, start_time))
}

fn read_proc_nul_fields(pid: u32, file: &str) -> Option<Vec<Vec<u8>>> {
    let bytes = fs::read(format!("/proc/{pid}/{file}")).ok()?;
    Some(parse_nul_fields(&bytes))
}

fn parse_nul_fields(bytes: &[u8]) -> Vec<Vec<u8>> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(values: &[&str]) -> Vec<Vec<u8>> {
        values
            .iter()
            .map(|value| value.as_bytes().to_vec())
            .collect()
    }

    #[test]
    fn parses_nul_fields_without_merging_arguments() {
        assert_eq!(
            parse_nul_fields(b"wine\0Z:\\Games\\RO\\ragexe.exe\0arg with spaces\0"),
            fields(&["wine", "Z:\\Games\\RO\\ragexe.exe", "arg with spaces"])
        );
    }

    #[test]
    fn exe_matching_uses_complete_basename_case_insensitively() {
        assert!(process_name_matches_exe("Ragexe.EXE", "ragexe.exe"));
        assert!(!process_name_matches_exe("not-ragexe.exe", "ragexe.exe"));
        assert!(process_name_matches_exe(
            "VeryLongClientE",
            "VeryLongClientExecutable.exe"
        ));
    }

    #[test]
    fn wrapper_argv_does_not_impersonate_the_game() {
        let wrapper = fields(&["python3", "/games/RO/ragexe.exe"]);
        assert!(!process_matches_exe(u32::MAX, &wrapper, "ragexe.exe"));
        let game = fields(&["C:\\Games\\RO\\ragexe.exe", "-1rag1"]);
        assert!(process_matches_exe(u32::MAX, &game, "ragexe.exe"));
    }

    #[test]
    fn prefix_cmdline_fallback_respects_path_boundaries() {
        assert!(cmdline_matches_prefix(
            &fields(&["--prefix=/prefix/abc"]),
            "/prefix/abc"
        ));
        assert!(cmdline_matches_prefix(
            &fields(&["/prefix/abc/drive_c/game.exe"]),
            "/prefix/abc"
        ));
        assert!(!cmdline_matches_prefix(
            &fields(&["--prefix=/prefix/abc2"]),
            "/prefix/abc"
        ));
    }

    #[test]
    fn explicit_prefix_rejects_unrelated_homonymous_process() {
        let cmdline = fields(&["wine", "/games/RO/ragexe.exe"]);
        let wrong_env = fields(&["WINEPREFIX=/prefix/other"]);

        assert_eq!(
            match_process_fields(&cmdline, &wrong_env, "/prefix/wanted", false, false),
            None
        );
    }

    #[test]
    fn explicit_prefix_accepts_environment_cmdline_or_descendant() {
        let cmdline = fields(&["wine", "/games/RO/ragexe.exe"]);
        let matching_env = fields(&["A=B=C", "WINEPREFIX=/prefix/wanted"]);
        let empty_env = Vec::new();

        assert_eq!(
            match_process_fields(&cmdline, &matching_env, "/prefix/wanted", false, false),
            Some("cmdline+prefix".into())
        );
        assert_eq!(
            match_process_fields(
                &fields(&["wine", "/prefix/wanted/ragexe.exe"]),
                &empty_env,
                "/prefix/wanted",
                false,
                false
            ),
            Some("cmdline+prefix".into())
        );
        assert_eq!(
            match_process_fields(&cmdline, &empty_env, "/prefix/wanted", false, true),
            Some("cmdline+child".into())
        );
    }

    #[test]
    fn empty_prefix_does_not_filter_by_environment() {
        assert_eq!(
            match_process_fields(
                &fields(&["wine", "/games/RO/ragexe.exe"]),
                &[],
                "",
                false,
                false
            ),
            Some("cmdline".into())
        );
    }

    #[test]
    fn proc_stat_parser_handles_spaces_and_closing_parenthesis_in_comm() {
        let stat = "42 (wine worker) name) S 7 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 12345 0";
        assert_eq!(parse_proc_stat(stat), Some((7, 12345)));
    }

    #[test]
    fn process_identity_verifies_current_process() {
        let identity = capture_process_identity(std::process::id()).unwrap();
        assert!(verify_process_identity(&identity));

        let stale = ProcessIdentity {
            start_time: identity.start_time.saturating_add(1),
            ..identity
        };
        assert!(!verify_process_identity(&stale));
    }
}
