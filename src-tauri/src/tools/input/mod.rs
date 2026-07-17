mod gateway;
mod uinput_worker;

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub use gateway::{GatewayWriter, InputGateway};
pub use uinput_worker::InputSource;

pub(crate) fn emit_status_if_changed<T>(
    app: &AppHandle,
    status_arc: &Arc<Mutex<T>>,
    event_name: &str,
    event: T,
) where
    T: Clone + PartialEq + Serialize,
{
    let mut prev = status_arc.lock().unwrap();
    if *prev != event {
        *prev = event.clone();
        drop(prev);
        let _ = app.emit(event_name, event);
    }
}
