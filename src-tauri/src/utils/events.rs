use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT_LOG: &str = "ro-launcher://log";
pub const EVENT_TOOL_LOG: &str = "ro-launcher://tool-log";
pub const EVENT_PROGRESS: &str = "ro-launcher://progress";
pub const EVENT_GAME_EXIT: &str = "ro-launcher://game-exit";
pub const EVENT_AUTOPOT_STATUS: &str = "ro-launcher://autopot-status";
pub const EVENT_AUTOBUFF_STATUS: &str = "ro-launcher://autobuff-status";
pub const EVENT_SPAMMER_STATUS: &str = "ro-launcher://spammer-status";

#[derive(Clone, Serialize)]
pub struct LogEvent {
    pub line: String,
}

#[derive(Clone, Serialize)]
pub struct ProgressEvent {
    pub step: String,
    pub percent: u32,
}

#[derive(Clone, Serialize)]
pub struct ExitEvent {
    pub code: i32,
}

pub fn emit_log(app: &AppHandle, line: impl Into<String>) -> Result<(), String> {
    app.emit(EVENT_LOG, LogEvent { line: line.into() })
        .map_err(|e| e.to_string())
}

fn emit_line_opt(app: Option<&AppHandle>, event: &str, line: impl Into<String>) {
    if let Some(app) = app {
        let _ = app.emit(event, LogEvent { line: line.into() });
    }
}

pub fn emit_log_opt(app: Option<&AppHandle>, line: impl Into<String>) {
    emit_line_opt(app, EVENT_LOG, line);
}

pub fn emit_tool_log_opt(app: Option<&AppHandle>, line: impl Into<String>) {
    emit_line_opt(app, EVENT_TOOL_LOG, line);
}

pub fn emit_progress(app: &AppHandle, step: &str, percent: u32) -> Result<(), String> {
    app.emit(
        EVENT_PROGRESS,
        ProgressEvent {
            step: step.to_string(),
            percent,
        },
    )
    .map_err(|e| e.to_string())
}
