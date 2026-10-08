use super::descriptor::{ArtifactDescriptor, ArtifactSource};

pub(crate) fn request_url_allowed(descriptor: &ArtifactDescriptor, url: &str) -> bool {
    matches!(
        descriptor.source,
        ArtifactSource::Https { url: catalog } if url == catalog
    ) && url.starts_with("https://")
}

pub(crate) fn catalog_url(descriptor: &ArtifactDescriptor) -> Option<&'static str> {
    match descriptor.source {
        ArtifactSource::Https { url } => Some(url),
        ArtifactSource::LocalOnly { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::descriptor::catalog_all;

    #[test]
    fn request_url_allowed_is_exact_catalog_https_only() {
        for descriptor in catalog_all() {
            let Some(url) = catalog_url(descriptor) else {
                assert!(!request_url_allowed(
                    descriptor,
                    "https://example.com/runtime"
                ));
                assert!(!request_url_allowed(descriptor, "file:///tmp/runtime"));
                continue;
            };
            assert!(request_url_allowed(descriptor, url));
            assert!(!request_url_allowed(
                descriptor,
                &url.replace("https://", "http://")
            ));
            assert!(!request_url_allowed(descriptor, "https://evil.example/x"));
            assert!(!request_url_allowed(descriptor, &format!("{url}?x=1")));
            assert!(!request_url_allowed(descriptor, "file:///tmp/x"));
        }
    }

    #[test]
    fn catalog_decision_ignores_appimage_env() {
        const CHILD_MARKER: &str = "RO_LAUNCHER_CATALOG_ENV_TEST_CHILD";
        if std::env::var_os(CHILD_MARKER).as_deref() == Some(std::ffi::OsStr::new("1")) {
            assert_eq!(std::env::var("APPIMAGE").unwrap(), "/tmp/fake.AppImage");
            assert_eq!(std::env::var("LD_LIBRARY_PATH").unwrap(), "/tmp/evil");
            let proton = &catalog_all()[1];
            let url = catalog_url(proton).unwrap();
            assert!(request_url_allowed(proton, url));
            return;
        }
        // Environment belongs to a subprocess; parallel lifecycle tests must not observe it.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tools::artifacts::source::tests::catalog_decision_ignores_appimage_env",
                "--nocapture",
            ])
            .env(CHILD_MARKER, "1")
            .env("APPIMAGE", "/tmp/fake.AppImage")
            .env("LD_LIBRARY_PATH", "/tmp/evil")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "child test did not execute"
        );
    }
}
