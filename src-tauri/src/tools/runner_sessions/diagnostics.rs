use sha2::{Digest, Sha256};
use std::path::Path;
use tauri::AppHandle;

use crate::utils::process::redact_sensitive_values;
use crate::utils::{emit_log_opt, emit_tool_log_opt, should_log_line};

pub const RUNNER_STDOUT_PREFIX: &str = "[runner:stdout] ";
pub const RUNNER_STDERR_PREFIX: &str = "[runner:stderr] ";
pub const SESSION_LOG_PREFIX: &str = "[Session] ";

pub fn path_log_token(path: &Path) -> String {
    let digest = Sha256::digest(path.to_string_lossy().as_bytes());
    format!("{:016x}", digest)[..16].to_string()
}

pub fn prefix_log_token(prefix: &Path) -> String {
    path_log_token(prefix)
}

pub fn emit_session_line(app: Option<&AppHandle>, line: impl Into<String>) {
    let line = line.into();
    emit_tool_log_opt(app, format!("{SESSION_LOG_PREFIX}{line}"));
}

pub fn handle_stderr_line(app: Option<&AppHandle>, line: &str, redactions: &[String]) {
    if let Some(rest) = line.strip_prefix(RUNNER_STDOUT_PREFIX) {
        let line = redact_sensitive_values(rest, redactions);
        if should_log_line(&line) {
            emit_log_opt(app, line);
        }
        return;
    }
    if let Some(rest) = line.strip_prefix(RUNNER_STDERR_PREFIX) {
        let line = redact_sensitive_values(rest, redactions);
        if should_log_line(&line) {
            emit_log_opt(app, line);
        }
        return;
    }
    let line = redact_sensitive_values(line, redactions);
    if !line.is_empty() {
        emit_tool_log_opt(app, format!("{SESSION_LOG_PREFIX}{line}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_runner_stdout_prefix() {
        let raw = "[runner:stdout] hello world";
        assert_eq!(raw.strip_prefix(RUNNER_STDOUT_PREFIX), Some("hello world"));
    }

    #[test]
    fn strips_runner_stderr_prefix() {
        let raw = "[runner:stderr] err:foo";
        assert_eq!(raw.strip_prefix(RUNNER_STDERR_PREFIX), Some("err:foo"));
    }

    #[test]
    fn redaction_before_runner_classification() {
        let secret = "s3cret";
        let line = format!("[runner:stdout] token={secret}");
        let redacted = redact_sensitive_values(
            line.strip_prefix(RUNNER_STDOUT_PREFIX).unwrap(),
            &[secret.to_string()],
        );
        assert!(!redacted.contains(secret));
        assert!(redacted.contains("<redacted>"));
    }
}
