use std::sync::{Arc, Mutex, Weak};

use ro_tools_linux::ProcessIdentity;

pub(crate) struct LeaseInner {
    registry: Weak<Mutex<super::RegistryInner>>,
    identity: ProcessIdentity,
    generation: u64,
}

pub struct MemoryLease {
    inner: LeaseInner,
}

impl MemoryLease {
    pub(crate) fn new(
        registry: &Arc<Mutex<super::RegistryInner>>,
        identity: ProcessIdentity,
        generation: u64,
    ) -> Self {
        Self {
            inner: LeaseInner {
                registry: Arc::downgrade(registry),
                identity,
                generation,
            },
        }
    }

    #[allow(dead_code)]
    pub fn identity(&self) -> ProcessIdentity {
        self.inner.identity
    }
}

impl Drop for MemoryLease {
    fn drop(&mut self) {
        let Some(registry) = self.inner.registry.upgrade() else {
            return;
        };
        let Ok(mut map) = registry.lock() else {
            return;
        };
        if let Some(entry) = map.sessions.get(&self.inner.identity) {
            if entry.generation == self.inner.generation {
                map.sessions.remove(&self.inner.identity);
            }
        }
    }
}
