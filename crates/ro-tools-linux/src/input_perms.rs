use std::fs::{self, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const INPUT_GROUP: &str = "input";
const INPUT_GROUP_HINT: &str = "sudo usermod -aG input $USER y reinicia sesión";
const RELOGIN_HINT: &str =
    "Estás en el grupo input pero los permisos no aplicaron. Cierra sesión y vuelve a entrar.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPermStatus {
    pub ok: bool,
    pub warning: Option<String>,
}

pub fn detect_input_permissions() -> InputPermStatus {
    if !is_member_of_group(INPUT_GROUP) {
        return InputPermStatus {
            ok: false,
            warning: Some(INPUT_GROUP_HINT.to_string()),
        };
    }

    if uinput_writable() && evdev_accessible() {
        return InputPermStatus {
            ok: true,
            warning: None,
        };
    }

    InputPermStatus {
        ok: true,
        warning: Some(RELOGIN_HINT.to_string()),
    }
}

pub fn detect_uinput_permissions() -> InputPermStatus {
    let uinput = uinput_writable();
    let evdev = evdev_accessible();
    if uinput && evdev {
        return InputPermStatus {
            ok: true,
            warning: None,
        };
    }

    let mut missing = Vec::new();
    if !uinput {
        missing.push("/dev/uinput no es escribible");
    }
    if !evdev {
        missing.push("/dev/input/event* no es legible");
    }
    InputPermStatus {
        ok: false,
        warning: Some(format!(
            "uinput no disponible: {}. {INPUT_GROUP_HINT}",
            missing.join("; ")
        )),
    }
}

fn current_username() -> Option<String> {
    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("LOGNAME").ok())
        .filter(|name| !name.is_empty())
}

fn is_member_of_group(group_name: &str) -> bool {
    let Ok(content) = fs::read_to_string("/etc/group") else {
        return false;
    };
    let Some(username) = current_username() else {
        return false;
    };
    let primary_gid = unsafe { libc::getgid() };

    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 4 || parts[0] != group_name {
            continue;
        }

        if parts[2].parse::<u32>().ok() == Some(primary_gid) {
            return true;
        }

        return parts[3]
            .split(',')
            .any(|member| !member.is_empty() && member == username);
    }

    false
}

fn uinput_writable() -> bool {
    OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/dev/uinput")
        .is_ok()
}

fn evdev_accessible() -> bool {
    let input_dir = Path::new("/dev/input");
    let Ok(entries) = fs::read_dir(input_dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("event") {
            continue;
        }
        if OpenOptions::new().read(true).open(&path).is_ok() {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_group_membership_from_content() {
        assert!(group_membership_from_content(
            "input:x:995:alice,bob",
            "input",
            "alice",
            1000
        ));
        assert!(!group_membership_from_content(
            "input:x:995:alice,bob",
            "input",
            "carol",
            1000
        ));
        assert!(group_membership_from_content(
            "input:x:995:alice",
            "input",
            "carol",
            995
        ));
    }

    fn group_membership_from_content(
        content: &str,
        group_name: &str,
        username: &str,
        primary_gid: u32,
    ) -> bool {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 || parts[0] != group_name {
                continue;
            }
            if parts[2].parse::<u32>().ok() == Some(primary_gid) {
                return true;
            }
            return parts[3].split(',').any(|member| member == username);
        }
        false
    }
}
