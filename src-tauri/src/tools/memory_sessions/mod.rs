mod classify;
mod lease;
mod messages;
mod reader;
mod session;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ro_tools_linux::{capture_process_identity, ProcessIdentity};

use crate::utils::emit_tool_log_opt;

pub use lease::MemoryLease;
pub use messages::{memory_access_preflight_token, memory_start_error, MEMORY_SESSION_MISSING};
pub use reader::SharedMemorySession;
pub use session::{MemoryAncestor, MemorySession};

pub(crate) struct Entry {
    generation: u64,
    session: Arc<MemorySession>,
}

#[derive(Default)]
pub(crate) struct RegistryInner {
    sessions: HashMap<ProcessIdentity, Entry>,
    next_generation: u64,
}

#[derive(Clone, Default)]
pub struct MemorySessionRegistry {
    inner: Arc<Mutex<RegistryInner>>,
}

impl MemorySessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        identity: ProcessIdentity,
        ancestor: MemoryAncestor,
    ) -> Result<MemoryLease, String> {
        if capture_process_identity(identity.pid).as_ref() != Some(&identity) {
            return Err("El proceso del cliente ya no es válido".into());
        }
        let session = Arc::new(MemorySession::open(identity, ancestor)?);
        let generation = {
            let mut map = self
                .inner
                .lock()
                .map_err(|_| "registro de memoria bloqueado".to_string())?;
            map.next_generation += 1;
            let generation = map.next_generation;
            map.sessions.insert(
                identity,
                Entry {
                    generation,
                    session: Arc::clone(&session),
                },
            );
            generation
        };
        let preflight = memory_access_preflight_token(session.access());
        emit_tool_log_opt(
            None,
            format!(
                "[Memory] client={}/{} backend={} preflight={}",
                identity.pid,
                identity.start_time,
                session.backend_label(),
                preflight,
            ),
        );
        Ok(MemoryLease::new(&self.inner, identity, generation))
    }

    pub fn get(&self, identity: &ProcessIdentity) -> Option<Arc<MemorySession>> {
        let map = self.inner.lock().ok()?;
        map.sessions
            .get(identity)
            .map(|entry| Arc::clone(&entry.session))
    }
}

pub fn launcher_memory_ancestor() -> MemoryAncestor {
    let pid = std::process::id();
    let identity = capture_process_identity(pid).expect("launcher process identity");
    MemoryAncestor::Launcher(identity)
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn lease_drop_removes_only_matching_generation() {
        let registry = MemorySessionRegistry::new();
        let identity = capture_process_identity(std::process::id()).expect("self process");
        let lease = registry
            .register(identity, launcher_memory_ancestor())
            .expect("register");
        assert!(registry.get(&identity).is_some());
        drop(lease);
        assert!(registry.get(&identity).is_none());
    }

    #[test]
    fn reregister_same_identity_keeps_newer_generation() {
        let registry = MemorySessionRegistry::new();
        let identity = capture_process_identity(std::process::id()).expect("self process");
        let old_lease = registry
            .register(identity, launcher_memory_ancestor())
            .expect("register");
        let first = registry.get(&identity).expect("session");
        let new_lease = registry
            .register(identity, launcher_memory_ancestor())
            .expect("register");
        let second = registry.get(&identity).expect("session");
        assert!(!Arc::ptr_eq(&first, &second));
        drop(old_lease);
        let third = registry
            .get(&identity)
            .expect("session after old lease drop");
        assert!(Arc::ptr_eq(&second, &third));
        drop(new_lease);
        assert!(registry.get(&identity).is_none());
    }

    #[test]
    fn repeated_get_returns_same_arc() {
        let registry = MemorySessionRegistry::new();
        let identity = capture_process_identity(std::process::id()).expect("self process");
        let _lease = registry
            .register(identity, launcher_memory_ancestor())
            .expect("register");
        let a = registry.get(&identity).expect("session");
        let b = registry.get(&identity).expect("session");
        let c = registry.get(&identity).expect("session");
        assert!(Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&b, &c));
    }
}
