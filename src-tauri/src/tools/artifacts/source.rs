use super::descriptor::{ArtifactDescriptor, ArtifactSource};

pub(crate) fn request_url_allowed(descriptor: &ArtifactDescriptor, url: &str) -> bool {
    matches!(
        descriptor.source,
        ArtifactSource::Https { url: catalog } if url == catalog
    ) && url.starts_with("https://")
}

pub(crate) fn catalog_url(descriptor: &ArtifactDescriptor) -> &'static str {
    match descriptor.source {
        ArtifactSource::Https { url } => url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::artifacts::descriptor::catalog_all;

    #[test]
    fn request_url_allowed_is_exact_catalog_https_only() {
        for descriptor in catalog_all() {
            let url = catalog_url(descriptor);
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
        std::env::set_var("APPIMAGE", "/tmp/fake.AppImage");
        std::env::set_var("LD_LIBRARY_PATH", "/tmp/evil");
        let proton = &catalog_all()[1];
        let url = catalog_url(proton);
        assert!(request_url_allowed(proton, url));
        std::env::remove_var("APPIMAGE");
        std::env::remove_var("LD_LIBRARY_PATH");
    }
}
