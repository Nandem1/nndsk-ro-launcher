use std::sync::{Arc, Mutex};

use crate::tools::autopot::AutopotHandle;
use crate::tools::input::YdotoolDaemon;

/// Estado compartido de la app entre comandos Tauri (juego, tools, input).
pub struct GameState {
    pub pid: Arc<Mutex<Option<u32>>>,
    pub autopot: AutopotHandle,
    pub ydotoold: Arc<YdotoolDaemon>,
}
