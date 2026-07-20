use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ro_tools_linux::ProcessIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchReservation {
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Idle,
    Launching {
        generation: u64,
        controller: Option<ProcessIdentity>,
        stop_requested: bool,
    },
    Running {
        identity: ProcessIdentity,
        controller: Option<ProcessIdentity>,
        generation: u64,
        stop_requested: bool,
    },
}

#[derive(Clone)]
pub struct GameProcessHandle {
    state: Arc<Mutex<ProcessState>>,
    next_generation: Arc<AtomicU64>,
}

impl GameProcessHandle {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProcessState::Idle)),
            next_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn begin_launch(&self) -> Result<LaunchReservation, String> {
        let mut state = self.lock()?;
        if *state != ProcessState::Idle {
            return Err("Ya hay un juego iniciándose o en ejecución".to_string());
        }
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        *state = ProcessState::Launching {
            generation,
            controller: None,
            stop_requested: false,
        };
        Ok(LaunchReservation { generation })
    }

    pub fn mark_running(
        &self,
        reservation: LaunchReservation,
        identity: ProcessIdentity,
    ) -> Result<(), String> {
        let mut state = self.lock()?;
        match *state {
            ProcessState::Launching {
                generation,
                controller,
                stop_requested: false,
            } if generation == reservation.generation => {
                *state = ProcessState::Running {
                    identity,
                    controller,
                    generation,
                    stop_requested: false,
                };
                Ok(())
            }
            ProcessState::Launching {
                generation,
                stop_requested: true,
                ..
            } if generation == reservation.generation => {
                Err("El lanzamiento fue cancelado por el usuario".to_string())
            }
            _ => Err("La reserva de lanzamiento ya no es válida".to_string()),
        }
    }

    pub fn mark_controller(
        &self,
        reservation: LaunchReservation,
        controller: ProcessIdentity,
    ) -> Result<(), String> {
        let mut state = self.lock()?;
        match *state {
            ProcessState::Launching {
                generation,
                stop_requested: false,
                ..
            } if generation == reservation.generation => {
                *state = ProcessState::Launching {
                    generation,
                    controller: Some(controller),
                    stop_requested: false,
                };
                Ok(())
            }
            ProcessState::Launching {
                generation,
                stop_requested: true,
                ..
            } if generation == reservation.generation => {
                Err("El lanzamiento fue cancelado por el usuario".to_string())
            }
            _ => Err("La reserva de lanzamiento ya no es válida".to_string()),
        }
    }

    pub fn cancel_launch(&self, reservation: LaunchReservation) {
        if let Ok(mut state) = self.state.lock() {
            if matches!(
                *state,
                ProcessState::Launching { generation, .. } if generation == reservation.generation
            ) {
                *state = ProcessState::Idle;
            }
        }
    }

    pub fn running_pid(&self) -> Result<Option<u32>, String> {
        match *self.lock()? {
            ProcessState::Idle => Ok(None),
            ProcessState::Launching { .. } => Err("El juego todavía se está iniciando".to_string()),
            ProcessState::Running { identity, .. } => Ok(Some(identity.pid)),
        }
    }

    pub fn request_stop(&self) -> Result<Vec<ProcessIdentity>, String> {
        let mut state = self.lock()?;
        let identities = match *state {
            ProcessState::Idle => Vec::new(),
            ProcessState::Launching {
                generation,
                controller,
                ..
            } => {
                *state = ProcessState::Launching {
                    generation,
                    controller,
                    stop_requested: true,
                };
                controller.into_iter().collect()
            }
            ProcessState::Running {
                identity,
                controller,
                generation,
                ..
            } => {
                *state = ProcessState::Running {
                    identity,
                    controller,
                    generation,
                    stop_requested: true,
                };
                let mut identities = vec![identity];
                if controller.is_some_and(|candidate| candidate != identity) {
                    identities.extend(controller);
                }
                identities
            }
        };
        Ok(identities)
    }

    pub fn stop_requested(&self, reservation: LaunchReservation) -> bool {
        let Ok(state) = self.state.lock() else {
            return true;
        };
        matches!(
            *state,
            ProcessState::Launching {
                generation,
                stop_requested: true,
                ..
            } | ProcessState::Running {
                generation,
                stop_requested: true,
                ..
            } if generation == reservation.generation
        )
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
        match *state {
            ProcessState::Running {
                identity,
                controller,
                generation,
                stop_requested: false,
            } if generation == reservation.generation && identity == expected => {
                *state = ProcessState::Running {
                    identity: replacement,
                    controller,
                    generation,
                    stop_requested: false,
                };
                true
            }
            _ => false,
        }
    }

    pub fn finish(&self, reservation: LaunchReservation) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if matches!(
            *state,
            ProcessState::Running { generation, .. } if generation == reservation.generation
        ) {
            *state = ProcessState::Idle;
            return true;
        }
        false
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ProcessState>, String> {
        self.state
            .lock()
            .map_err(|_| "El estado del proceso del juego está bloqueado".to_string())
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

    #[test]
    fn rejects_parallel_launches_and_recovers_after_cancel() {
        let process = GameProcessHandle::new();
        let first = process.begin_launch().unwrap();
        assert!(process.begin_launch().is_err());
        process.cancel_launch(first);
        assert!(process.begin_launch().is_ok());
    }

    #[test]
    fn only_the_current_generation_can_finish_the_process() {
        let process = GameProcessHandle::new();
        let first = process.begin_launch().unwrap();
        process.mark_running(first, identity(42)).unwrap();

        assert_eq!(process.running_pid().unwrap(), Some(42));
        assert!(process.finish(first));
        assert_eq!(process.running_pid().unwrap(), None);

        let second = process.begin_launch().unwrap();
        process.mark_running(second, identity(84)).unwrap();
        assert!(!process.finish(first));
        assert_eq!(process.running_pid().unwrap(), Some(84));
    }

    #[test]
    fn stop_request_cancels_launch_and_tracks_controller_identity() {
        let process = GameProcessHandle::new();
        let reservation = process.begin_launch().unwrap();
        process.mark_controller(reservation, identity(21)).unwrap();

        assert_eq!(process.request_stop().unwrap(), vec![identity(21)]);
        assert!(process.stop_requested(reservation));
        assert!(process.mark_running(reservation, identity(42)).is_err());
        process.cancel_launch(reservation);
        assert!(process.begin_launch().is_ok());
    }

    #[test]
    fn handoff_replaces_only_the_expected_live_generation() {
        let process = GameProcessHandle::new();
        let reservation = process.begin_launch().unwrap();
        process.mark_running(reservation, identity(42)).unwrap();

        assert!(!process.replace_running(reservation, identity(41), identity(84)));
        assert!(process.replace_running(reservation, identity(42), identity(84)));
        assert_eq!(process.running_pid().unwrap(), Some(84));

        process.request_stop().unwrap();
        assert!(!process.replace_running(reservation, identity(84), identity(126)));
    }
}
