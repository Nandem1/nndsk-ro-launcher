//! Infraestructura del launcher (Wine, paths, eventos Tauri).
//!
//! Dominio de tools (AutoPot, input) vive en [`crate::tools`] + crates `ro-tools-*`.

pub mod audio;
mod events;
mod fs;
mod game_dir;
pub mod gecko;
mod json;
mod operation_lock;
mod paths;
mod prefix;
pub mod process;
mod runner;
mod servers;
mod settings;
mod system;
mod webview;
mod wine;

pub use events::*;
pub use fs::*;
pub use game_dir::*;
pub use json::*;
pub use operation_lock::*;
pub use paths::*;
pub use prefix::*;
pub use runner::*;
pub use servers::*;
pub use settings::*;
pub use system::*;
pub use webview::*;
pub use wine::*;

use tauri::AppHandle;
use tokio::io::AsyncBufReadExt;

pub fn should_log_line(line: &str) -> bool {
    !line.contains("fixme:") && !line.contains("libEGL warning")
}

fn default_stderr_lines(line: &str) -> Vec<String> {
    if should_log_line(line) {
        vec![line.to_string()]
    } else {
        vec![]
    }
}

/// Drena stdout/stderr del proceso hijo y emite líneas filtradas al frontend.
pub async fn drain_and_log(app: &AppHandle, child: &mut tokio::process::Child) {
    drain_child_output(app, child, default_stderr_lines).await;
}

/// Igual que [`drain_and_log`] pero con procesador custom para stderr (p. ej. hints de audio).
pub async fn drain_child_output<F>(
    app: &AppHandle,
    child: &mut tokio::process::Child,
    stderr_lines: F,
) where
    F: Fn(&str) -> Vec<String> + Send + Sync + 'static,
{
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let app_out = app.clone();
    let h_out = tokio::spawn(async move {
        if let Some(out) = stdout {
            let mut lines = tokio::io::BufReader::new(out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if should_log_line(&line) {
                    emit_log_opt(Some(&app_out), line);
                }
            }
        }
    });

    let app_err = app.clone();
    let h_err = tokio::spawn(async move {
        if let Some(err) = stderr {
            let mut lines = tokio::io::BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                for emitted in stderr_lines(&line) {
                    emit_log_opt(Some(&app_err), emitted);
                }
            }
        }
    });

    let _ = tokio::join!(h_out, h_err);
}
