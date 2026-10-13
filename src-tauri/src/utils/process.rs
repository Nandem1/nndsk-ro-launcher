use tauri::AppHandle;
use tokio::process::Command;

use crate::utils::{drain_and_log, pipe_output};

#[allow(dead_code)]
pub async fn run_logged_command(
    app: &AppHandle,
    mut cmd: Command,
    error_context: &str,
) -> Result<i32, String> {
    pipe_output(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Error al ejecutar {error_context}: {e}"))?;

    drain_and_log(app, &mut child).await;

    let status = child.wait().await.map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(-1))
}

#[allow(dead_code)]
pub async fn run_logged_command_ok(
    app: &AppHandle,
    cmd: Command,
    error_context: &str,
) -> Result<(), String> {
    let code = run_logged_command(app, cmd, error_context).await?;
    if code != 0 {
        return Err(format!("{error_context} falló con código: {code}"));
    }
    Ok(())
}

pub fn redact_sensitive_values(line: &str, sensitive_values: &[String]) -> String {
    let mut values: Vec<_> = sensitive_values
        .iter()
        .filter(|value| !value.is_empty())
        .map(String::as_str)
        .collect();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    values
        .into_iter()
        .fold(line.to_string(), |redacted, value| {
            redacted.replace(value, "<redacted>")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_every_sensitive_value_without_touching_empty_values() {
        let values = vec!["hunter2".to_string(), "alice".to_string(), String::new()];
        assert_eq!(
            redact_sensitive_values("user=alice password=hunter2", &values),
            "user=<redacted> password=<redacted>"
        );
    }

    #[test]
    fn redacts_overlapping_values_longest_first() {
        let values = vec!["alice".to_string(), "alice123".to_string()];
        assert_eq!(
            redact_sensitive_values("password=alice123 user=alice", &values),
            "password=<redacted> user=<redacted>"
        );
    }
}
