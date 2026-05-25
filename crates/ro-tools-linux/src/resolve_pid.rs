use ro_tools_core::ClientProfile;

use crate::proc_memory::{address_in_maps, ProcMemoryReader};
use crate::wine_process::{find_game_processes, normalize_prefix};

/// Selecciona el mejor PID del cliente RO validando memoria cuando es posible.
pub fn resolve_best_game_pid(
    launcher_pid: u32,
    exe_path: &str,
    wine_prefix: &str,
    profile: &ClientProfile,
) -> Option<(u32, String)> {
    let prefix = normalize_prefix(wine_prefix);
    let candidates = find_game_processes(launcher_pid, exe_path, &prefix);

    for candidate in &candidates {
        let Ok(reader) = ProcMemoryReader::open(candidate.pid) else {
            continue;
        };

        let mapped = address_in_maps(candidate.pid, profile.hp_base);
        let probe = reader.probe_stats(profile.hp_base);

        match probe {
            Ok((cur_hp, max_hp, cur_sp, max_sp)) if looks_like_stats(max_hp, max_sp) => {
                return Some((
                    candidate.pid,
                    format!(
                        "{} | map={mapped} HP={cur_hp}/{max_hp} SP={cur_sp}/{max_sp}",
                        candidate.reason
                    ),
                ));
            }
            Ok((cur_hp, max_hp, _, _)) if max_hp > 0 || cur_hp > 0 => {
                return Some((
                    candidate.pid,
                    format!(
                        "{} | map={mapped} HP={cur_hp}/{max_hp} (parcial)",
                        candidate.reason
                    ),
                ));
            }
            Err(_) if mapped => {
                return Some((
                    candidate.pid,
                    format!(
                        "{} | map=true (lectura falló, reintentando)",
                        candidate.reason
                    ),
                ));
            }
            _ => {}
        }
    }

    candidates
        .first()
        .map(|c| (c.pid, format!("{} (sin validar memoria)", c.reason)))
}

fn looks_like_stats(max_hp: u32, max_sp: u32) -> bool {
    (1_000..=500_000).contains(&max_hp) && max_sp > 0 && max_sp <= 100_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_sanity_check() {
        assert!(looks_like_stats(50_000, 500));
        assert!(!looks_like_stats(0, 500));
        assert!(!looks_like_stats(50_000, 0));
    }
}
