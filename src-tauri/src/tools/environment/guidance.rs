use crate::models::environment::HostFamily;

pub(super) fn host_family() -> HostFamily {
    std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .map(|value| family_from_os_release(&value))
        .unwrap_or(HostFamily::Other)
}

fn family_from_os_release(value: &str) -> HostFamily {
    let arch = value
        .lines()
        .filter_map(|line| line.split_once('='))
        .any(|(key, value)| {
            matches!(key.trim(), "ID" | "ID_LIKE")
                && value
                    .trim()
                    .trim_matches(['\'', '"'])
                    .split_whitespace()
                    .any(|id| id == "arch")
        });
    if arch {
        HostFamily::Arch
    } else {
        HostFamily::Other
    }
}

/// Package names are curated data, never values taken from errors or shell input.
pub(super) fn packages_for(code: &str, family: HostFamily) -> &'static [&'static str] {
    if family != HostFamily::Arch {
        return &[];
    }
    match code {
        "host-cabextract" => &["cabextract"],
        "host-unzip" => &["unzip"],
        "host-downloader" => &["curl"],
        "host-winetricks" => &["winetricks"],
        "host-python" => &["python"],
        "host-ca" => &["ca-certificates", "ca-certificates-utils"],
        "host-vulkan" => &["vulkan-icd-loader"],
        "host-elf32" => &["lib32-glibc"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_uses_declared_family_not_desktop_name() {
        assert_eq!(family_from_os_release("ID=arch\n"), HostFamily::Arch);
        assert_eq!(
            family_from_os_release("ID=ryoku\nID_LIKE=\"arch linux\"\n"),
            HostFamily::Arch
        );
        assert_eq!(
            family_from_os_release("NAME=Arch\nID=other"),
            HostFamily::Other
        );
        assert_eq!(family_from_os_release("ID=archish"), HostFamily::Other);
        assert_eq!(family_from_os_release(""), HostFamily::Other);
        assert!(packages_for("host-unzip", HostFamily::Other).is_empty());
        assert!(packages_for("host-$(secret)", HostFamily::Arch).is_empty());
    }
}
