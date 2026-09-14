use crate::process::{
    join_all_stream_handles, join_finished_stream_handles, launch_controller, signal_descendants,
    validate_spec_for_launch, waitpid_reap_with_signal, LaunchControllerError,
};
use crate::protocol::SharedWriter;
use ro_session_protocol::{clamp_grace_ms, SessionEvent, SessionRequest, MAX_IN_FLIGHT_LAUNCHES};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::process::LaunchOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    Ready,
    Stopping,
    Stopped,
}

pub struct ControllerRecord {
    #[allow(dead_code)]
    request_id: String,
    #[allow(dead_code)]
    controller_pid: u32,
    pub is_shutdown_controller: bool,
    streams: Option<LaunchOutcome>,
}

pub struct Supervisor {
    pub phase: SessionPhase,
    owned_prefix: PathBuf,
    _parent_pid: u32,
    base_env: HashMap<String, String>,
    stderr_writer: Arc<Mutex<File>>,
    writer: SharedWriter,
    request_ids: HashSet<String>,
    in_flight: usize,
    pid_to_request: HashMap<u32, String>,
    controllers: HashMap<String, ControllerRecord>,
    shutdown_request_id: Option<String>,
    shutdown_deadline: Option<Instant>,
    shutdown_grace_ms: u64,
    forced_shutdown: bool,
    term_sent_at: Option<Instant>,
    kill_sent: bool,
    pub last_wait_echild: bool,
    pending_streams: Vec<JoinHandle<()>>,
}

impl Supervisor {
    pub fn new(
        owned_prefix: PathBuf,
        _parent_pid: u32,
        base_env: HashMap<String, String>,
        stderr_writer: Arc<Mutex<File>>,
        writer: SharedWriter,
    ) -> Self {
        Self {
            phase: SessionPhase::Ready,
            owned_prefix,
            _parent_pid,
            base_env,
            stderr_writer,
            writer,
            request_ids: HashSet::new(),
            in_flight: 0,
            pid_to_request: HashMap::new(),
            controllers: HashMap::new(),
            shutdown_request_id: None,
            shutdown_deadline: None,
            shutdown_grace_ms: 2_000,
            forced_shutdown: false,
            term_sent_at: None,
            kill_sent: false,
            last_wait_echild: false,
            pending_streams: Vec::new(),
        }
    }

    pub fn join_finished_streams(&mut self) {
        join_finished_stream_handles(&mut self.pending_streams);
    }

    pub fn join_all_pending_streams(&mut self) {
        join_all_stream_handles(&mut self.pending_streams);
    }

    #[cfg(test)]
    pub fn set_in_flight_for_test(&mut self, value: usize) {
        self.in_flight = value;
    }

    #[cfg(test)]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight
    }

    #[cfg(test)]
    pub fn forced_shutdown_for_test(&self) -> bool {
        self.forced_shutdown
    }

    pub fn handle_line(&mut self, line: &str) -> Result<(), String> {
        let request: SessionRequest =
            serde_json::from_str(line).map_err(|e| format!("invalid JSON: {e}"))?;
        match request {
            SessionRequest::Hello { .. } => {
                self.emit_validation_error(None, "second Hello rejected");
                self.begin_forced_shutdown();
                Ok(())
            }
            SessionRequest::Launch { request_id, spec } => self.handle_launch(request_id, spec),
            SessionRequest::Shutdown {
                request_id,
                shutdown_spec,
                grace_ms,
            } => self.handle_shutdown(request_id, shutdown_spec, grace_ms),
        }
    }

    fn handle_launch(
        &mut self,
        request_id: String,
        spec: ro_session_protocol::ProcessSpec,
    ) -> Result<(), String> {
        if self.phase != SessionPhase::Ready {
            self.emit_validation_error(Some(request_id), "Launch rejected while not Ready");
            return Ok(());
        }
        if !self.request_ids.insert(request_id.clone()) {
            self.emit_validation_error(Some(request_id), "duplicate request_id");
            return Ok(());
        }
        if self.in_flight >= MAX_IN_FLIGHT_LAUNCHES {
            self.emit_validation_error(Some(request_id), "too many in-flight launches");
            return Ok(());
        }

        let merged = match validate_spec_for_launch(&spec, &self.owned_prefix, &self.base_env) {
            Ok(env) => env,
            Err(message) => {
                self.emit_validation_error(Some(request_id), &message);
                return Ok(());
            }
        };

        let sidecar_pid = std::process::id();
        let request_id_for_spawn = request_id.clone();
        let outcome = match launch_controller(
            &spec,
            &merged,
            self.stderr_writer.clone(),
            sidecar_pid,
            |controller_pid| {
                self.pid_to_request
                    .insert(controller_pid, request_id_for_spawn.clone());
            },
        ) {
            Ok(outcome) => outcome,
            Err(err) => {
                self.emit_launch_error(Some(request_id), &err);
                return Ok(());
            }
        };

        let controller_pid = outcome.controller_pid;
        self.pid_to_request
            .insert(controller_pid, request_id.clone());
        self.in_flight += 1;
        self.controllers.insert(
            request_id.clone(),
            ControllerRecord {
                request_id: request_id.clone(),
                controller_pid,
                is_shutdown_controller: false,
                streams: Some(outcome),
            },
        );

        self.writer
            .emit(&SessionEvent::LaunchAccepted {
                request_id,
                controller_pid,
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn handle_shutdown(
        &mut self,
        request_id: String,
        shutdown_spec: ro_session_protocol::ProcessSpec,
        grace_ms: u64,
    ) -> Result<(), String> {
        if self.phase == SessionPhase::Stopping {
            self.emit_validation_error(Some(request_id), "second Shutdown rejected");
            return Ok(());
        }
        if self.phase != SessionPhase::Ready {
            self.emit_validation_error(Some(request_id), "Shutdown rejected");
            return Ok(());
        }
        if !self.request_ids.insert(request_id.clone()) {
            self.emit_validation_error(Some(request_id), "duplicate request_id");
            return Ok(());
        }

        self.phase = SessionPhase::Stopping;
        self.shutdown_request_id = Some(request_id.clone());
        self.shutdown_grace_ms = clamp_grace_ms(grace_ms);

        if let Err(message) =
            validate_spec_for_launch(&shutdown_spec, &self.owned_prefix, &self.base_env)
        {
            self.emit_stage_error(Some(request_id.clone()), "shutdown", &message, None);
            self.begin_forced_shutdown();
            return Ok(());
        }

        let merged = validate_spec_for_launch(&shutdown_spec, &self.owned_prefix, &self.base_env)
            .map_err(|e| e.to_string())?;
        let sidecar_pid = std::process::id();
        let request_id_for_spawn = request_id.clone();
        match launch_controller(
            &shutdown_spec,
            &merged,
            self.stderr_writer.clone(),
            sidecar_pid,
            |controller_pid| {
                self.pid_to_request
                    .insert(controller_pid, request_id_for_spawn.clone());
            },
        ) {
            Ok(outcome) => {
                let controller_pid = outcome.controller_pid;
                self.controllers.insert(
                    request_id.clone(),
                    ControllerRecord {
                        request_id: request_id.clone(),
                        controller_pid,
                        is_shutdown_controller: true,
                        streams: Some(outcome),
                    },
                );
                self.shutdown_deadline =
                    Some(Instant::now() + Duration::from_millis(self.shutdown_grace_ms));
                self.writer
                    .emit(&SessionEvent::ShutdownAccepted { request_id })
                    .map_err(|e| e.to_string())?;
            }
            Err(err) => {
                self.emit_stage_error(Some(request_id), "shutdown", &err.message, err.errno);
                self.begin_forced_shutdown();
            }
        }
        Ok(())
    }

    pub fn begin_forced_shutdown(&mut self) {
        if self.phase == SessionPhase::Stopped {
            return;
        }
        self.forced_shutdown = true;
        self.phase = SessionPhase::Stopping;
        if self.shutdown_deadline.is_none() {
            self.shutdown_deadline = Some(Instant::now() + Duration::from_millis(2_000));
        }
    }

    pub fn drain_reap_events(&mut self) {
        loop {
            match waitpid_reap_with_signal() {
                Ok(Some((pid, exit_code, signal))) => {
                    self.last_wait_echild = false;
                    let request_id = self.pid_to_request.remove(&pid);
                    if let Some(request_id) = request_id {
                        let is_shutdown = self
                            .controllers
                            .get(&request_id)
                            .map(|r| r.is_shutdown_controller)
                            .unwrap_or(false);
                        if !is_shutdown {
                            self.in_flight = self.in_flight.saturating_sub(1);
                            let rid = request_id.clone();
                            let _ = self.writer.emit(&SessionEvent::ControllerExited {
                                request_id: rid,
                                controller_pid: pid,
                                exit_code,
                                signal,
                            });
                        }
                        if let Some(record) = self.controllers.get_mut(&request_id) {
                            if let Some(mut streams) = record.streams.take() {
                                self.pending_streams.extend(streams.take_stream_handles());
                            }
                        }
                        self.controllers.remove(&request_id);
                    }
                }
                Ok(None) => {
                    self.last_wait_echild = false;
                    break;
                }
                Err(()) => {
                    self.last_wait_echild = true;
                    break;
                }
            }
        }
    }

    pub fn advance_shutdown(&mut self) {
        if self.phase != SessionPhase::Stopping {
            return;
        }
        self.drain_reap_events();

        let deadline = self.shutdown_deadline.unwrap_or_else(Instant::now);
        let now = Instant::now();

        if self.forced_shutdown || now >= deadline {
            if self.term_sent_at.is_none() {
                self.term_sent_at = Some(now);
            }
            signal_descendants(std::process::id(), libc::SIGTERM);
        }

        if let Some(term_at) = self.term_sent_at {
            if term_at.elapsed() >= Duration::from_secs(2) {
                signal_descendants(std::process::id(), libc::SIGKILL);
                self.kill_sent = true;
            }
        }

        self.drain_reap_events();

        if self.term_sent_at.is_some() && self.last_wait_echild {
            self.join_all_pending_streams();
            self.phase = SessionPhase::Stopped;
        }
    }

    fn emit_launch_error(&self, request_id: Option<String>, err: &LaunchControllerError) {
        let _ = self.writer.emit(&SessionEvent::Error {
            request_id,
            stage: "launch".into(),
            errno: err.errno,
            message: err.message.clone(),
        });
    }

    fn emit_validation_error(&self, request_id: Option<String>, message: &str) {
        let _ = self.writer.emit(&SessionEvent::Error {
            request_id,
            stage: "validation".into(),
            errno: None,
            message: message.into(),
        });
    }

    fn emit_stage_error(
        &self,
        request_id: Option<String>,
        stage: &str,
        message: &str,
        errno: Option<i32>,
    ) {
        let _ = self.writer.emit(&SessionEvent::Error {
            request_id,
            stage: stage.into(),
            errno,
            message: message.into(),
        });
    }

    #[cfg(test)]
    fn term_sent(&self) -> bool {
        self.term_sent_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::EventWriter;
    use ro_session_protocol::{
        EnvironmentChange, ProcessSpec, MAX_IN_FLIGHT_LAUNCHES, PROTOCOL_VERSION,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn parse_events(lines: &[String]) -> Vec<SessionEvent> {
        lines
            .iter()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn test_supervisor(prefix: PathBuf) -> (Supervisor, Arc<Mutex<Vec<String>>>) {
        let stderr = fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("/dev/null");
        let (writer, lines) = EventWriter::collecting();
        let sup = Supervisor::new(
            prefix,
            std::process::id(),
            HashMap::new(),
            Arc::new(Mutex::new(stderr)),
            writer,
        );
        (sup, lines)
    }

    fn true_spec(prefix: &Path) -> ProcessSpec {
        ProcessSpec {
            program: "/usr/bin/true".into(),
            args: Vec::new(),
            cwd: prefix.display().to_string(),
            env: vec![EnvironmentChange {
                key: "WINEPREFIX".into(),
                value: Some(prefix.display().to_string()),
            }],
        }
    }

    #[test]
    fn second_hello_starts_forced_shutdown_not_stopped() {
        let dir =
            std::env::temp_dir().join(format!("ro-sessiond-unit-hello-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let prefix = crate::process::canonicalize_prefix(&dir).unwrap();
        let (mut sup, lines) = test_supervisor(prefix);
        let line = format!(
            r#"{{"type":"hello","protocolVersion":{}}}"#,
            PROTOCOL_VERSION
        );
        sup.handle_line(&line).unwrap();
        assert_eq!(sup.phase, SessionPhase::Stopping);
        assert!(sup.forced_shutdown_for_test());
        assert_ne!(sup.phase, SessionPhase::Stopped);
        let events = parse_events(&lines.lock().unwrap());
        assert!(events.iter().any(|ev| matches!(
            ev,
            SessionEvent::Error {
                stage,
                ..
            } if stage == "validation"
        )));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_request_id_keeps_ready() {
        let dir = std::env::temp_dir().join(format!("ro-sessiond-unit-dup-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let prefix = crate::process::canonicalize_prefix(&dir).unwrap();
        let (mut sup, lines) = test_supervisor(prefix.clone());
        let rid = "550e8400-e29b-41d4-a716-446655440099".to_string();
        let launch = serde_json::to_string(&SessionRequest::Launch {
            request_id: rid.clone(),
            spec: true_spec(&prefix),
        })
        .unwrap();
        sup.handle_line(&launch).unwrap();
        sup.handle_line(&launch).unwrap();
        assert_eq!(sup.phase, SessionPhase::Ready);
        let events = parse_events(&lines.lock().unwrap());
        assert!(events.iter().any(|ev| matches!(
            ev,
            SessionEvent::Error {
                stage,
                ..
            } if stage == "validation"
        )));
        sup.drain_reap_events();
        sup.join_all_pending_streams();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn max_in_flight_launch_rejected() {
        let dir = std::env::temp_dir().join(format!("ro-sessiond-unit-max-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let prefix = crate::process::canonicalize_prefix(&dir).unwrap();
        let (mut sup, lines) = test_supervisor(prefix.clone());
        sup.set_in_flight_for_test(MAX_IN_FLIGHT_LAUNCHES);
        let overflow = serde_json::to_string(&SessionRequest::Launch {
            request_id: "00000000-0000-4000-8000-000000000099".into(),
            spec: true_spec(&prefix),
        })
        .unwrap();
        sup.handle_line(&overflow).unwrap();
        assert_eq!(sup.in_flight_count(), MAX_IN_FLIGHT_LAUNCHES);
        let events = parse_events(&lines.lock().unwrap());
        assert!(events.iter().any(|ev| matches!(
            ev,
            SessionEvent::Error {
                stage,
                ..
            } if stage == "validation"
        )));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_does_not_term_before_grace_deadline() {
        let dir =
            std::env::temp_dir().join(format!("ro-sessiond-unit-grace-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let prefix = crate::process::canonicalize_prefix(&dir).unwrap();
        let (mut sup, _lines) = test_supervisor(prefix.clone());
        let shutdown = serde_json::to_string(&SessionRequest::Shutdown {
            request_id: "550e8400-e29b-41d4-a716-446655440088".into(),
            shutdown_spec: true_spec(&prefix),
            grace_ms: 10_000,
        })
        .unwrap();
        sup.handle_line(&shutdown).unwrap();
        assert_eq!(sup.phase, SessionPhase::Stopping);
        assert_eq!(sup.shutdown_grace_ms, 10_000);
        sup.advance_shutdown();
        assert!(!sup.term_sent());
        sup.drain_reap_events();
        sup.join_all_pending_streams();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_grace_ms_clamped_to_minimum() {
        let dir =
            std::env::temp_dir().join(format!("ro-sessiond-unit-clamp-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let prefix = crate::process::canonicalize_prefix(&dir).unwrap();
        let (mut sup, _lines) = test_supervisor(prefix.clone());
        let shutdown = serde_json::to_string(&SessionRequest::Shutdown {
            request_id: "550e8400-e29b-41d4-a716-446655440077".into(),
            shutdown_spec: true_spec(&prefix),
            grace_ms: 1,
        })
        .unwrap();
        sup.handle_line(&shutdown).unwrap();
        assert_eq!(sup.shutdown_grace_ms, 1_000);
        sup.drain_reap_events();
        sup.join_all_pending_streams();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shutdown_invalid_spec_forces_term() {
        let dir =
            std::env::temp_dir().join(format!("ro-sessiond-unit-bad-shut-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let prefix = crate::process::canonicalize_prefix(&dir).unwrap();
        let (mut sup, lines) = test_supervisor(prefix.clone());
        let bad_spec = ProcessSpec {
            program: "/no/such/ro-sessiond-missing".into(),
            args: Vec::new(),
            cwd: prefix.display().to_string(),
            env: vec![EnvironmentChange {
                key: "WINEPREFIX".into(),
                value: Some(prefix.display().to_string()),
            }],
        };
        let shutdown = serde_json::to_string(&SessionRequest::Shutdown {
            request_id: "550e8400-e29b-41d4-a716-446655440066".into(),
            shutdown_spec: bad_spec,
            grace_ms: 10_000,
        })
        .unwrap();
        sup.handle_line(&shutdown).unwrap();
        assert!(sup.forced_shutdown_for_test());
        let events = parse_events(&lines.lock().unwrap());
        assert!(events.iter().any(|ev| matches!(
            ev,
            SessionEvent::Error {
                stage,
                ..
            } if stage == "shutdown"
        )));
        sup.advance_shutdown();
        assert!(sup.term_sent());
        let _ = fs::remove_dir_all(&dir);
    }
}
