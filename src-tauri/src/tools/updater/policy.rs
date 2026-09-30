use tauri::{AppHandle, Manager};

use crate::models::update::{UpdateFailureClass, UpdateUnavailableReason};
use crate::tools::updater::backend::BackendError;

/// Proof that no game client is launching or running.
/// Private field so callers cannot forge it without `attest`.
#[derive(Debug, Clone, Copy)]
pub struct IdleClients(());

impl IdleClients {
    pub fn attest(active_count: usize) -> Result<Self, usize> {
        if active_count == 0 {
            Ok(Self(()))
        } else {
            Err(active_count)
        }
    }
}

pub fn packaged_appimage(app: &AppHandle) -> Result<std::path::PathBuf, UpdateUnavailableReason> {
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        app.env()
            .appimage
            .map(std::path::PathBuf::from)
            .ok_or(UpdateUnavailableReason::NotPackaged)
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        let _ = app;
        Err(UpdateUnavailableReason::UnsupportedOs)
    }
}

pub fn classify_backend_error(error: BackendError) -> (UpdateFailureClass, String) {
    match error {
        BackendError::Unauthentic(message) => (UpdateFailureClass::Unauthentic, message),
        BackendError::Unreachable(message) => (UpdateFailureClass::Unreachable, message),
        BackendError::Install(message) => (UpdateFailureClass::Install, message),
        BackendError::Other(message) => (UpdateFailureClass::Install, message),
    }
}

/// Map plugin errors. Signature / pubkey / minisign failures → Unauthentic.
/// Never map those to Unreachable. Never mention SHA-256 as a fallback.
pub fn classify_plugin_error(error: tauri_plugin_updater::Error) -> BackendError {
    use tauri_plugin_updater::Error;
    match error {
        Error::AuthenticationFailed
        | Error::Minisign(_)
        | Error::SignatureUtf8(_)
        | Error::Base64(_)
        | Error::SignedVersionMismatch { .. }
        | Error::MissingSignedVersion => BackendError::Unauthentic(error.to_string()),
        Error::Network(_)
        | Error::Reqwest(_)
        | Error::ReleaseNotFound
        | Error::TargetNotFound(_)
        | Error::TargetsNotFound(_)
        | Error::EmptyEndpoints
        | Error::InsecureTransportProtocol
        | Error::Http(_)
        | Error::InvalidHeaderValue(_)
        | Error::InvalidHeaderName(_)
        | Error::UrlParse(_) => BackendError::Unreachable(error.to_string()),
        Error::UnsupportedOs => BackendError::Other(error.to_string()),
        other => BackendError::Install(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_clients_attest_only_zero() {
        assert!(IdleClients::attest(0).is_ok());
        assert_eq!(IdleClients::attest(1).err(), Some(1));
        assert_eq!(IdleClients::attest(3).err(), Some(3));
    }

    #[test]
    fn signature_failures_are_unauthentic_not_unreachable() {
        let classified = classify_plugin_error(tauri_plugin_updater::Error::AuthenticationFailed);
        assert!(matches!(classified, BackendError::Unauthentic(_)));

        let network =
            classify_plugin_error(tauri_plugin_updater::Error::Network("timeout".to_string()));
        assert!(matches!(network, BackendError::Unreachable(_)));

        let replay = classify_plugin_error(tauri_plugin_updater::Error::SignedVersionMismatch {
            signed: "0.1.0".into(),
            announced: "9.9.9".into(),
        });
        assert!(matches!(replay, BackendError::Unauthentic(_)));
    }
}
