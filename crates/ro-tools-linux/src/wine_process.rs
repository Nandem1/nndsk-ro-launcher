use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GameProcessCandidate {
    pub pid: u32,
    pub reason: String,
}

/// Resolve the PID of the RO client inside a Wine session.
pub fn resolve_game_pid(
    launcher_pid: u32,
    exe_path: &str,
    wine_prefix: &str,
) -> Option<u32> {
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
    let exe_name = Path::new(exe_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(exe_path);
    let prefix_norm = normalize_prefix(wine_prefix);

    let mut candidates = Vec::new();

    if let Some(reason) = match_process(launcher_pid, exe_name, &prefix_norm, launcher_pid) {
        candidates.push(GameProcessCandidate {
            pid: launcher_pid,
            reason,
        });
    }

    let proc_dir = fs::read_dir("/proc").ok();
    if let Some(proc_dir) = proc_dir {
        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let Some(pid_str) = name.to_str() else {
                continue;
            };
            if !pid_str.chars().all(|c| c.is_ascii_digit()) {
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

    candidates.sort_by_key(|c| score_candidate(c, launcher_pid));
    candidates
}

fn score_candidate(candidate: &GameProcessCandidate, launcher_pid: u32) -> u32 {
    let mut score = 0;
    if candidate.pid == launcher_pid {
        score += 100;
    }
    if is_descendant_of(candidate.pid, launcher_pid) {
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

fn match_process(
    pid: u32,
    exe_name: &str,
    prefix_norm: &str,
    launcher_pid: u32,
) -> Option<String> {
    let cmdline = read_proc_file(pid, "cmdline");
    if cmdline.is_empty() {
        return None;
    }

    let cmdline_lower = cmdline.to_ascii_lowercase();
    let exe_lower = exe_name.to_ascii_lowercase();
    if !cmdline_lower.contains(&exe_lower) {
        return None;
    }

    let environ = read_proc_file(pid, "environ");
    let mut reasons = vec!["cmdline".to_string()];

    if prefix_matches(&environ, prefix_norm) || cmdline.contains(prefix_norm) {
        reasons.push("prefix".into());
    }

    if pid == launcher_pid {
        reasons.push("launcher".into());
    } else if is_descendant_of(pid, launcher_pid) {
        reasons.push("child".into());
    }

    Some(reasons.join("+"))
}

fn prefix_matches(environ: &str, prefix_norm: &str) -> bool {
    if prefix_norm.is_empty() {
        return true;
    }
    for entry in environ.split('\0').filter(|s| !s.is_empty()) {
        if let Some(value) = entry.strip_prefix("WINEPREFIX=") {
            if normalize_prefix(value) == prefix_norm {
                return true;
            }
        }
    }
    false
}

pub fn normalize_prefix(prefix: &str) -> String {
    fs::canonicalize(prefix)
        .map(|p| p.to_string_lossy().to_string())
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
        if ppid <= 1 {
            return false;
        }
        current = ppid;
    }
    false
}

fn read_ppid(pid: u32) -> Result<u32, ()> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| ())?;
    let rparen = stat.rfind(')').ok_or(())?;
    let rest: Vec<&str> = stat[rparen + 1..].split_whitespace().collect();
    rest.get(1).and_then(|s| s.parse().ok()).ok_or(())
}

fn read_proc_file(pid: u32, file: &str) -> String {
    let path = PathBuf::from(format!("/proc/{pid}/{file}"));
    fs::read(&path)
        .map(|bytes| {
            bytes
                .split(|&b| b == 0)
                .filter(|part| !part.is_empty())
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect::<Vec<_>>()
                .join("\0")
        })
        .unwrap_or_default()
}
