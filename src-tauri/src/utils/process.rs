use tauri::AppHandle;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use crate::utils::audio::{self, mmdevapi_recovery_hint};
use crate::utils::{drain_and_log, pipe_output, should_log_line};

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

fn game_stderr_lines(line: &str) -> Vec<String> {
    let mut out = Vec::new();

    if audio::is_mmdevapi_audio_error(line) {
        out.push(mmdevapi_recovery_hint().to_string());
    }
    if line.contains("err:") || (should_log_line(line) && !line.is_empty()) {
        out.push(line.to_string());
    }

    out
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

pub async fn drain_game_streams_redacted(
    app: AppHandle,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    sensitive_values: Vec<String>,
) {
    let stderr_sensitive_values = sensitive_values.clone();
    let app_out = app.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = redact_sensitive_values(&line, &stderr_sensitive_values);
                if should_log_line(&line) {
                    crate::utils::emit_log_opt(Some(&app_out), line);
                }
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = redact_sensitive_values(&line, &sensitive_values);
                for emitted in game_stderr_lines(&line) {
                    crate::utils::emit_log_opt(Some(&app), emitted);
                }
            }
        }
    });
    let _ = tokio::join!(stdout_task, stderr_task);
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
