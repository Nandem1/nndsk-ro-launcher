use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

use crate::models::installation::{CreateInstallation, InstallationDraft, InstallationStatus};
use crate::utils::{data_file, load_json_recovering, write_json, OperationGuard};

#[derive(Default)]
struct State {
    drafts: Option<Vec<InstallationDraft>>,
    active: HashMap<String, watch::Sender<bool>>,
    closing: bool,
}

#[derive(Clone)]
pub struct InstallationManager {
    state: Arc<Mutex<State>>,
    path: PathBuf,
}

impl Default for InstallationManager {
    fn default() -> Self {
        Self::new(data_file("installations.json"))
    }
}

impl InstallationManager {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            path,
        }
    }

    fn transaction<T>(
        &self,
        mutate: impl FnOnce(&mut State) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "El registro de instalaciones está bloqueado")?;
        if state.drafts.is_none() {
            let loaded = load_json_recovering(&self.path, validate_drafts)?;
            let mut drafts: Vec<InstallationDraft> = loaded.value;
            let mut recovered = false;
            for draft in &mut drafts {
                if draft.status.is_active() {
                    draft.status = InstallationStatus::Interrupted;
                    draft.message = Some("La sesión anterior se interrumpió. Los archivos se conservaron; puedes volver a ejecutar el instalador".into());
                    recovered = true;
                }
            }
            if recovered {
                write_json(&self.path, &drafts)?;
            }
            state.drafts = Some(drafts);
        }
        mutate(&mut state)
    }

    pub fn list(&self) -> Result<Vec<InstallationDraft>, String> {
        self.transaction(|state| Ok(state.drafts.as_ref().unwrap().clone()))
    }

    pub fn get(&self, id: &str) -> Result<InstallationDraft, String> {
        self.transaction(|state| {
            state
                .drafts
                .as_ref()
                .unwrap()
                .iter()
                .find(|draft| draft.id == id)
                .cloned()
                .ok_or_else(|| "La instalación no existe".into())
        })
    }

    pub fn create(&self, input: CreateInstallation) -> Result<InstallationDraft, String> {
        let runners = crate::tools::runners::discover_runners()?;
        if !runners.iter().any(|runner| runner.path == input.runner) {
            return Err("Selecciona nndsk-ro-proton o Wine 7.16 del catálogo administrado".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let draft = InstallationDraft {
            server_id: format!("installed-{id}"),
            id,
            name: input.name.trim().into(),
            installer_path: input.installer_path,
            destination_path: input.destination_path,
            runner: input.runner,
            require_webview2: input.require_webview2,
            status: InstallationStatus::Pending,
            binding: None,
            exit_code: None,
            message: None,
        };
        draft.validate()?;
        if !std::path::Path::new(&draft.installer_path).is_file() {
            return Err("El instalador seleccionado no existe".into());
        }
        self.transaction(|state| {
            if state.closing {
                return Err("El launcher se está cerrando".into());
            }
            let mut next = state.drafts.as_ref().unwrap().clone();
            next.push(draft.clone());
            write_json(&self.path, &next)?;
            state.drafts = Some(next);
            Ok(draft)
        })
    }

    pub(crate) fn update(
        &self,
        id: &str,
        mutate: impl FnOnce(&mut InstallationDraft) -> Result<(), String>,
    ) -> Result<InstallationDraft, String> {
        self.transaction(|state| {
            let mut next = state.drafts.as_ref().unwrap().clone();
            let draft = next
                .iter_mut()
                .find(|draft| draft.id == id)
                .ok_or("La instalación no existe")?;
            mutate(draft)?;
            draft.validate()?;
            let result = draft.clone();
            write_json(&self.path, &next)?;
            state.drafts = Some(next);
            Ok(result)
        })
    }

    pub(crate) fn reserve(
        &self,
        id: &str,
    ) -> Result<(InstallationDraft, watch::Receiver<bool>, ActiveInstallation), String> {
        // Confirmation holds this admission through async validation and both writes.
        // A restart must not invalidate its snapshot or race a server insertion.
        let _admission = OperationGuard::acquire("installation-confirm", std::path::Path::new(id))?;
        self.transaction(|state| {
            if state.closing {
                return Err("El launcher se está cerrando".into());
            }
            if state.active.contains_key(id) {
                return Err("La instalación ya está en curso".into());
            }
            let mut next = state.drafts.as_ref().unwrap().clone();
            let draft = next
                .iter_mut()
                .find(|draft| draft.id == id)
                .ok_or("La instalación no existe")?;
            if !draft.status.can_restart() {
                return Err("Esta instalación no necesita ejecutarse de nuevo".into());
            }
            draft.status = InstallationStatus::Preparing;
            draft.exit_code = None;
            draft.message = None;
            let draft = draft.clone();
            write_json(&self.path, &next)?;
            state.drafts = Some(next);
            let (send, receive) = watch::channel(false);
            state.active.insert(id.to_string(), send);
            Ok((
                draft,
                receive,
                ActiveInstallation {
                    manager: self.clone(),
                    id: id.into(),
                },
            ))
        })
    }

    pub fn cancel(&self, id: &str) -> Result<InstallationDraft, String> {
        self.transaction(|state| {
            let send = state.active.get(id).ok_or("La instalación no está en ejecución")?;
            // Cancellation is delivered even if the subsequent status write fails.
            send.send_replace(true);
            let mut next = state.drafts.as_ref().unwrap().clone();
            let draft = next.iter_mut().find(|draft| draft.id == id).ok_or("La instalación no existe")?;
            draft.status = InstallationStatus::Stopping;
            draft.message = Some("Deteniendo la instalación. Si el entorno se está preparando, se cerrará al finalizar ese paso".into());
            let result = draft.clone();
            write_json(&self.path, &next)?;
            state.drafts = Some(next);
            Ok(result)
        })
    }

    pub(crate) fn is_active(&self, id: &str) -> Result<bool, String> {
        self.transaction(|state| Ok(state.active.contains_key(id)))
    }

    pub fn request_shutdown(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closing = true;
            for sender in state.active.values() {
                sender.send_replace(true);
            }
        }
    }

    pub async fn wait_for_shutdown(&self) {
        let wait = async {
            loop {
                if self
                    .state
                    .lock()
                    .map(|state| state.active.is_empty())
                    .unwrap_or(true)
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        };
        // RunnerSessionRegistry owns and completes subprocess shutdown independently.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(15), wait).await;
    }
}

pub(crate) struct ActiveInstallation {
    manager: InstallationManager,
    id: String,
}

impl Drop for ActiveInstallation {
    fn drop(&mut self) {
        let recovery = if self
            .manager
            .get(&self.id)
            .is_ok_and(|draft| draft.status.is_active())
        {
            self.manager
                .update(&self.id, |draft| {
                    if draft.status.is_active() {
                        draft.status = InstallationStatus::Interrupted;
                        draft.message = Some(
                            "La ejecución se interrumpió. Se conservaron los archivos y el entorno"
                                .into(),
                        );
                    }
                    Ok(())
                })
                .map(|_| ())
        } else {
            Ok(())
        };
        if let Ok(mut state) = self.manager.state.lock() {
            if let Err(error) = recovery {
                if let Some(draft) = state
                    .drafts
                    .as_mut()
                    .and_then(|drafts| drafts.iter_mut().find(|draft| draft.id == self.id))
                {
                    draft.status = InstallationStatus::Interrupted;
                    draft.message = Some(format!(
                        "La instalación se interrumpió y no se pudo guardar su estado: {error}"
                    ));
                }
            }
            state.active.remove(&self.id);
        }
    }
}

fn validate_drafts(drafts: &Vec<InstallationDraft>) -> Result<(), String> {
    let mut ids = HashSet::new();
    for draft in drafts {
        draft.validate()?;
        if !ids.insert(&draft.id) {
            return Err("Hay instalaciones duplicadas".into());
        }
    }
    Ok(())
}
