use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ro_tools_linux::ProcessIdentity;

use crate::models::game_client::{GameClientSnapshot, GameClientStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchReservation {
    generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClientMetadata {
    client_id: String,
    server_id: String,
    server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProcessState {
    Launching {
        metadata: ClientMetadata,
        controller: Option<ProcessIdentity>,
        stop_requested: bool,
    },
    Running {
        metadata: ClientMetadata,
        identity: ProcessIdentity,
        controller: Option<ProcessIdentity>,
        stop_requested: bool,
    },
}

impl ProcessState {
    fn metadata(&self) -> &ClientMetadata {
        match self {
            Self::Launching { metadata, .. } | Self::Running { metadata, .. } => metadata,
        }
    }

    fn identity(&self) -> Option<ProcessIdentity> {
        match self {
            Self::Launching { .. } => None,
            Self::Running { identity, .. } => Some(*identity),
        }
    }

    fn snapshot(&self) -> GameClientSnapshot {
        let metadata = self.metadata();
        let (status, pid) = match self {
            Self::Launching { stop_requested, .. } => (
                if *stop_requested {
                    GameClientStatus::Stopping
                } else {
                    GameClientStatus::Launching
                },
                None,
            ),
            Self::Running {
                identity,
                stop_requested,
                ..
            } => (
                if *stop_requested {
                    GameClientStatus::Stopping
                } else {
                    GameClientStatus::Running
                },
                Some(identity.pid),
            ),
        };
        GameClientSnapshot {
            client_id: metadata.client_id.clone(),
            server_id: metadata.server_id.clone(),
            server_name: metadata.server_name.clone(),
            status,
            pid,
        }
    }
}

#[derive(Default)]
struct RegistryState {
    clients: HashMap<u64, ProcessState>,
}

pub struct StopRequest {
    pub identities: Vec<ProcessIdentity>,
    pub was_only_client: bool,
}

pub struct FinishedClient {
    pub client_id: String,
    pub server_id: String,
    pub server_name: String,
    pub stop_requested: bool,
    pub remaining_clients: usize,
}

#[derive(Clone)]
pub struct GameProcessHandle {
    state: Arc<Mutex<RegistryState>>,
    next_generation: Arc<AtomicU64>,
}

impl GameProcessHandle {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RegistryState::default())),
            next_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn begin_launch(
        &self,
        client_id: String,
        server_id: String,
        server_name: String,
    ) -> Result<LaunchReservation, String> {
        let mut state = self.lock()?;
        if state
            .clients
            .values()
            .any(|client| matches!(client, ProcessState::Launching { .. }))
        {
            return Err("Ya hay otro cliente iniciándose; espera a que termine".to_string());
        }
        if state
            .clients
            .values()
            .any(|client| client.metadata().client_id == client_id)
        {
            return Err("El identificador de cliente ya está en uso".to_string());
        }
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        state.clients.insert(
            generation,
            ProcessState::Launching {
                metadata: ClientMetadata {
                    client_id,
                    server_id,
                    server_name,
                },
                controller: None,
                stop_requested: false,
            },
        );
        Ok(LaunchReservation { generation })
    }

    pub fn mark_running(
        &self,
        reservation: LaunchReservation,
        identity: ProcessIdentity,
    ) -> Result<GameClientSnapshot, String> {
        let mut state = self.lock()?;
        if state.clients.iter().any(|(generation, client)| {
            *generation != reservation.generation && client.identity() == Some(identity)
        }) {
            return Err("El proceso detectado ya pertenece a otro cliente".to_string());
        }
        let client = state
            .clients
            .get_mut(&reservation.generation)
            .ok_or_else(|| "La reserva de lanzamiento ya no es válida".to_string())?;
        match client {
            ProcessState::Launching {
                metadata,
                controller,
                stop_requested: false,
            } => {
                *client = ProcessState::Running {
                    metadata: metadata.clone(),
                    identity,
                    controller: *controller,
                    stop_requested: false,
                };
                Ok(client.snapshot())
            }
            ProcessState::Launching {
                stop_requested: true,
                ..
            } => Err("El lanzamiento fue cancelado por el usuario".to_string()),
            ProcessState::Running { .. } => {
                Err("La reserva de lanzamiento ya está en ejecución".to_string())
            }
        }
    }

    pub fn mark_controller(
        &self,
        reservation: LaunchReservation,
        controller_identity: ProcessIdentity,
    ) -> Result<(), String> {
        let mut state = self.lock()?;
        let client = state
            .clients
            .get_mut(&reservation.generation)
            .ok_or_else(|| "La reserva de lanzamiento ya no es válida".to_string())?;
        match client {
            ProcessState::Launching {
                controller,
                stop_requested: false,
                ..
            } => {
                *controller = Some(controller_identity);
                Ok(())
            }
            ProcessState::Launching {
                stop_requested: true,
                ..
            } => Err("El lanzamiento fue cancelado por el usuario".to_string()),
            ProcessState::Running { .. } => {
                Err("La reserva de lanzamiento ya está en ejecución".to_string())
            }
        }
    }

    pub fn cancel_launch(&self, reservation: LaunchReservation) {
        if let Ok(mut state) = self.state.lock() {
            if matches!(
                state.clients.get(&reservation.generation),
                Some(ProcessState::Launching { .. })
            ) {
                state.clients.remove(&reservation.generation);
            }
        }
    }

    pub fn active_count(&self) -> Result<usize, String> {
        Ok(self.lock()?.clients.len())
    }

    pub fn snapshots(&self) -> Result<Vec<GameClientSnapshot>, String> {
        let state = self.lock()?;
        let mut clients: Vec<_> = state
            .clients
            .iter()
            .map(|(generation, client)| (*generation, client.snapshot()))
            .collect();
        clients.sort_by_key(|(generation, _)| *generation);
        Ok(clients.into_iter().map(|(_, client)| client).collect())
    }

    pub fn sole_running_pid(&self) -> Result<u32, String> {
        self.sole_running_pid_for_server(None)
    }

    pub fn sole_running_pid_for(&self, server_id: &str) -> Result<u32, String> {
        self.sole_running_pid_for_server(Some(server_id))
    }

    fn sole_running_pid_for_server(&self, server_id: Option<&str>) -> Result<u32, String> {
        let state = self.lock()?;
        if state.clients.is_empty() {
            return Err("No hay ningún cliente en ejecución".to_string());
        }
        if state.clients.len() != 1 {
            return Err(
                "Las herramientas sólo están disponibles cuando hay exactamente un cliente abierto"
                    .to_string(),
            );
        }
        let client = state.clients.values().next().expect("len checked");
        let (metadata, identity) = match client {
            ProcessState::Running {
                metadata,
                identity,
                stop_requested: false,
                ..
            } => (metadata, identity),
            ProcessState::Running {
                stop_requested: true,
                ..
            } => return Err("El cliente se está cerrando".to_string()),
            ProcessState::Launching { .. } => {
                return Err("El cliente todavía se está iniciando".to_string());
            }
        };
        if server_id.is_some_and(|expected| expected != metadata.server_id) {
            return Err(format!(
                "El cliente activo pertenece a {}; selecciona ese servidor para usar herramientas",
                metadata.server_name
            ));
        }
        Ok(identity.pid)
    }

    pub fn request_stop(&self, client_id: &str) -> Result<StopRequest, String> {
        let mut state = self.lock()?;
        let was_only_client = state.clients.len() == 1;
        let generation = state
            .clients
            .iter()
            .find_map(|(generation, client)| {
                (client.metadata().client_id == client_id).then_some(*generation)
            })
            .ok_or_else(|| "El cliente ya no está en ejecución".to_string())?;
        let client = state
            .clients
            .get_mut(&generation)
            .expect("generation found");
        let identities = match client {
            ProcessState::Launching {
                controller,
                stop_requested,
                ..
            } => {
                *stop_requested = true;
                controller.iter().copied().collect()
            }
            ProcessState::Running {
                identity,
                controller,
                stop_requested,
                ..
            } => {
                *stop_requested = true;
                let mut identities = vec![*identity];
                if controller.is_some_and(|candidate| candidate != *identity) {
                    identities.extend(*controller);
                }
                identities
            }
        };
        Ok(StopRequest {
            identities,
            was_only_client,
        })
    }

    pub fn request_stop_all(&self) -> Result<Vec<ProcessIdentity>, String> {
        let mut state = self.lock()?;
        let mut identities = HashSet::new();
        for client in state.clients.values_mut() {
            match client {
                ProcessState::Launching {
                    controller,
                    stop_requested,
                    ..
                } => {
                    *stop_requested = true;
                    identities.extend(controller.iter().copied());
                }
                ProcessState::Running {
                    identity,
                    controller,
                    stop_requested,
                    ..
                } => {
                    *stop_requested = true;
                    identities.insert(*identity);
                    identities.extend(controller.iter().copied());
                }
            }
        }
        Ok(identities.into_iter().collect())
    }

    pub fn stop_requested(&self, reservation: LaunchReservation) -> bool {
        let Ok(state) = self.state.lock() else {
            return true;
        };
        matches!(
            state.clients.get(&reservation.generation),
            Some(
                ProcessState::Launching {
                    stop_requested: true,
                    ..
                } | ProcessState::Running {
                    stop_requested: true,
                    ..
                }
            )
        )
    }

    pub fn candidate_available_for_handoff(
        &self,
        reservation: LaunchReservation,
        candidate: ProcessIdentity,
    ) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        if !matches!(
            state.clients.get(&reservation.generation),
            Some(ProcessState::Running {
                stop_requested: false,
                ..
            })
        ) {
            return false;
        }
        if state.clients.iter().any(|(generation, client)| {
            *generation != reservation.generation
                && (matches!(client, ProcessState::Launching { .. })
                    || client.identity() == Some(candidate))
        }) {
            return false;
        }
        true
    }

    pub fn replace_running(
        &self,
        reservation: LaunchReservation,
        expected: ProcessIdentity,
        replacement: ProcessIdentity,
    ) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.clients.iter().any(|(generation, client)| {
            *generation != reservation.generation
                && (matches!(client, ProcessState::Launching { .. })
                    || client.identity() == Some(replacement))
        }) {
            return false;
        }
        let Some(client) = state.clients.get_mut(&reservation.generation) else {
            return false;
        };
        match client {
            ProcessState::Running {
                identity,
                stop_requested: false,
                ..
            } if *identity == expected => {
                *identity = replacement;
                true
            }
            _ => false,
        }
    }

    pub fn finish(&self, reservation: LaunchReservation) -> Option<FinishedClient> {
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        if !matches!(
            state.clients.get(&reservation.generation),
            Some(ProcessState::Running { .. })
        ) {
            return None;
        }
        let ProcessState::Running {
            metadata,
            stop_requested,
            ..
        } = state.clients.remove(&reservation.generation)?
        else {
            unreachable!("running state checked before removal");
        };
        Some(FinishedClient {
            client_id: metadata.client_id,
            server_id: metadata.server_id,
            server_name: metadata.server_name,
            stop_requested,
            remaining_clients: state.clients.len(),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, RegistryState>, String> {
        self.state
            .lock()
            .map_err(|_| "El registro de clientes está bloqueado".to_string())
    }
}

impl Default for GameProcessHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(pid: u32) -> ProcessIdentity {
        ProcessIdentity {
            pid,
            start_time: u64::from(pid),
        }
    }

    fn launch(process: &GameProcessHandle, id: &str) -> LaunchReservation {
        process
            .begin_launch(id.into(), "server".into(), "Server".into())
            .unwrap()
    }

    #[test]
    fn permits_multiple_running_clients_but_serializes_detection() {
        let process = GameProcessHandle::new();
        let first = launch(&process, "first");
        assert!(process
            .begin_launch("parallel".into(), "server".into(), "Server".into())
            .is_err());
        process.mark_running(first, identity(42)).unwrap();

        let second = launch(&process, "second");
        process.mark_running(second, identity(84)).unwrap();

        assert_eq!(process.snapshots().unwrap().len(), 2);
        assert!(process.sole_running_pid().is_err());
    }

    #[test]
    fn cancellation_only_removes_the_requested_launch() {
        let process = GameProcessHandle::new();
        let first = launch(&process, "first");
        process.cancel_launch(first);
        let second = launch(&process, "second");
        process.mark_running(second, identity(84)).unwrap();
        assert_eq!(process.snapshots().unwrap()[0].client_id, "second");
    }

    #[test]
    fn stop_and_finish_are_scoped_to_one_client() {
        let process = GameProcessHandle::new();
        let first = launch(&process, "first");
        process.mark_running(first, identity(42)).unwrap();
        let second = launch(&process, "second");
        process.mark_running(second, identity(84)).unwrap();

        let stop = process.request_stop("first").unwrap();
        assert_eq!(stop.identities, vec![identity(42)]);
        assert!(!stop.was_only_client);
        let finished = process.finish(first).unwrap();
        assert!(finished.stop_requested);
        assert_eq!(finished.remaining_clients, 1);
        assert_eq!(process.sole_running_pid().unwrap(), 84);
    }

    #[test]
    fn handoff_never_claims_another_client_or_races_a_launch() {
        let process = GameProcessHandle::new();
        let first = launch(&process, "first");
        process.mark_running(first, identity(42)).unwrap();

        let second = launch(&process, "second");
        assert!(!process.candidate_available_for_handoff(first, identity(84)));
        process.mark_running(second, identity(84)).unwrap();
        assert!(!process.candidate_available_for_handoff(first, identity(84)));
        assert!(!process.replace_running(first, identity(42), identity(84)));
    }

    #[test]
    fn tools_require_one_running_client_from_the_selected_server() {
        let process = GameProcessHandle::new();
        let reservation = process
            .begin_launch("first".into(), "sakura".into(), "Sakura".into())
            .unwrap();
        process.mark_running(reservation, identity(42)).unwrap();

        assert_eq!(process.sole_running_pid_for("sakura").unwrap(), 42);
        assert!(process.sole_running_pid_for("other").is_err());
    }
}
