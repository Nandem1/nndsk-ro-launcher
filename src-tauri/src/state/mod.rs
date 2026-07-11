use std::sync::{Arc, Mutex};

use crate::tools::autobuff::AutobuffHandle;
use crate::tools::autopot::AutopotHandle;
use crate::tools::input::{InputGateway, YdotoolDaemon};
use crate::tools::spammer::SpammerHandle;

/// Estado compartido de la app entre comandos Tauri (juego, tools, input).
pub struct GameState {
    pub pid: Arc<Mutex<Option<u32>>>,
    pub autopot: AutopotHandle,
    pub autobuff: AutobuffHandle,
    pub spammer: SpammerHandle,
    pub input: InputGateway,
    pub ydotoold: Arc<YdotoolDaemon>,
}
