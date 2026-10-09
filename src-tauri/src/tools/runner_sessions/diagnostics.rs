use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tauri::AppHandle;
use tokio::sync::Notify;

use crate::utils::process::redact_sensitive_values;
use crate::utils::{emit_log_opt, emit_tool_log_opt, should_log_line};

pub const RUNNER_STDOUT_PREFIX: &str = "[runner:stdout] ";
pub const RUNNER_STDERR_PREFIX: &str = "[runner:stderr] ";
pub const SESSION_LOG_PREFIX: &str = "[Session] ";

/// A small redacted diagnostic window, not an inference that every Wine error is fatal.
/// Setup holds an exclusive prefix guard, so its runner commands cannot overlap other tools.
#[derive(Default)]
pub(crate) struct FailureOutput {
    first_error: Option<String>,
    tail: VecDeque<String>,
}

impl FailureOutput {
    fn record(&mut self, line: &str) {
        let line: String = line.chars().take(1024).collect();
        // Empty Steam Runtime validation emits CRITICAL on a normal cold install. It is not
        // the cause. Keep the first explicit ERROR even if later missing-file errors follow.
        if self.first_error.is_none() && line.contains("ERROR:") {
            self.first_error = Some(line.clone());
        }
        self.tail.push_back(line);
        if self.tail.len() > 8 {
            self.tail.pop_front();
        }
    }

    pub(crate) fn annotate(&self, primary: String) -> String {
        let mut lines = Vec::new();
        if let Some(first) = &self.first_error {
            lines.push(first.as_str());
        }
        lines.extend(
            self.tail
                .iter()
                .map(String::as_str)
                .filter(|line| Some(*line) != self.first_error.as_deref()),
        );
        if lines.is_empty() {
            return primary;
        }
        format!(
            "{primary}\nSalida del runner (diagnóstico):\n{}",
            lines.join("\n")
        )
    }
}

#[derive(Default)]
pub(crate) struct RunnerOutputHub {
    subscribers: Mutex<Vec<Weak<Mutex<FailureOutput>>>>,
    closed: AtomicBool,
    notify: Notify,
}

impl RunnerOutputHub {
    pub(crate) fn subscribe(&self) -> Arc<Mutex<FailureOutput>> {
        let sink = Arc::new(Mutex::new(FailureOutput::default()));
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|sink| sink.strong_count() > 0);
        subscribers.push(Arc::downgrade(&sink));
        sink
    }

    pub(crate) fn record(&self, line: &str, redactions: &[String]) {
        let Some(line) = line
            .strip_prefix(RUNNER_STDERR_PREFIX)
            .or_else(|| line.strip_prefix(RUNNER_STDOUT_PREFIX))
        else {
            return;
        };
        if line.is_empty() || !should_log_line(line) {
            return;
        }
        let redacted = redact_sensitive_values(line, redactions);
        self.subscribers.lock().unwrap().retain(|sink| {
            if let Some(sink) = sink.upgrade() {
                sink.lock().unwrap().record(&redacted);
                true
            } else {
                false
            }
        });
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub(crate) async fn wait_closed(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.closed.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
}

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
    fn diagnostic_retains_first_causal_candidate_but_not_normal_cold_validation() {
        let hub = RunnerOutputHub::default();
        let sink = hub.subscribe();
        hub.record("[runner:stderr] CRITICAL: steamrt4 validation failed", &[]);
        hub.record(
            "[runner:stderr] ERROR: Digest mismatched secret",
            &["secret".into()],
        );
        for _ in 0..1000 {
            hub.record("[runner:stderr] later secondary error", &[]);
        }
        let message = sink.lock().unwrap().annotate("Inicialización falló".into());
        assert!(message.contains("ERROR: Digest mismatched <redacted>"));
        assert!(!message.contains("secret"));
        assert!(!message.contains("CRITICAL"));
        assert!(message.len() < 10_000);
        drop(sink);
        let retry = hub.subscribe();
        assert_eq!(
            retry.lock().unwrap().annotate("new failure".into()),
            "new failure"
        );
    }

    #[tokio::test]
    async fn diagnostic_eof_can_be_observed_after_it_already_arrived() {
        let hub = RunnerOutputHub::default();
        hub.close();
        tokio::time::timeout(std::time::Duration::from_millis(100), hub.wait_closed())
            .await
            .unwrap();
    }

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
