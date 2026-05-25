//! Reglas de dgVoodoo embebidas en el launcher (sin I/O).

pub const TEMPLATE_FILES: &[&str] = &[
    "D3DImm.dll",
    "DDraw.dll",
    "dgVoodoo.conf",
    "dgVoodooCpl.exe",
];

pub const REQUIRED_FILES: &[&str] = &["D3DImm.dll", "DDraw.dll", "dgVoodoo.conf"];

pub fn validate_conf(content: &str, issues: &mut Vec<String>) {
    if conf_value_anywhere(content, "Version").is_none() {
        issues.push("dgVoodoo.conf no parece válido (falta Version)".to_string());
    }

    if let Some(output_api) = conf_value(content, "General", "OutputAPI") {
        let api = output_api.to_ascii_lowercase();
        if api.is_empty() || api == "disabled" {
            issues.push("OutputAPI no está configurado en dgVoodoo.conf".to_string());
        }
    } else {
        issues.push("OutputAPI no definido en dgVoodoo.conf".to_string());
    }

    if let Some(pass_through) = conf_value(content, "DirectX", "DisableAndPassThru") {
        if pass_through.eq_ignore_ascii_case("true") {
            issues.push(
                "DisableAndPassThru está activo — dgVoodoo no interceptará DirectX".to_string(),
            );
        }
    }
}

fn conf_value_anywhere(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn conf_value(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let current = &line[1..line.len() - 1];
            in_section = current.eq_ignore_ascii_case(section);
            continue;
        }
        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_missing_output_api() {
        let mut issues = Vec::new();
        validate_conf("[General]\nVersion = 2", &mut issues);
        assert!(issues.iter().any(|i| i.contains("OutputAPI")));
    }

    #[test]
    fn accepts_valid_conf() {
        let content = "[General]\nVersion = 2\nOutputAPI = d3d11_fl11\n[DirectX]\nDisableAndPassThru = false";
        let mut issues = Vec::new();
        validate_conf(content, &mut issues);
        assert!(issues.is_empty());
    }
}
