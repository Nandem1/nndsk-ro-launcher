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

#[derive(Default)]
pub struct ServerRepository {
    lock: Mutex<()>,
}

impl ServerRepository {
    pub fn list(&self) -> Result<Vec<crate::models::server::ServerConfig>, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "El repositorio de servidores está bloqueado".to_string())?;
        crate::utils::read_json(&crate::utils::servers_path())
    }

    pub fn save(&self, servers: &[crate::models::server::ServerConfig]) -> Result<(), String> {
        crate::models::server::validate_servers(servers)?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "El repositorio de servidores está bloqueado".to_string())?;
        crate::utils::write_json(&crate::utils::servers_path(), servers)
    }
}
