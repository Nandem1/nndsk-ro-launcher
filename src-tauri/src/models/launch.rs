use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub const MAX_LAUNCH_ARGUMENTS: usize = 64;
pub const MAX_LAUNCH_ARGUMENT_LENGTH: usize = 4_096;
pub const MAX_LAUNCH_TOTAL_LENGTH: usize = 16_384;
pub const MAX_LAUNCH_FIELDS: usize = 16;
pub const MAX_LAUNCH_FIELD_KEY_LENGTH: usize = 32;
pub const MAX_LAUNCH_VALUE_LENGTH: usize = 4_096;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchStrategy {
    #[default]
    Direct,
    Patcher,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    #[serde(default)]
    pub strategy: LaunchStrategy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patcher_args: Vec<String>,
    /// Fuerza WebView2 cuando el cliente lo carga dinámicamente o su PE está protegido.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_webview2: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl LaunchConfig {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn active_args(&self) -> &[String] {
        match self.strategy {
            LaunchStrategy::Direct => &self.game_args,
            LaunchStrategy::Patcher => &self.patcher_args,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_argument_list("del juego", &self.game_args)?;
        validate_argument_list("del patcher", &self.patcher_args)?;
        Ok(())
    }

    pub fn render_args(&self, values: &LaunchValues) -> Result<Vec<String>, String> {
        self.validate()?;
        render_launch_args(self.active_args(), values)
    }
}

/// Valores efímeros recibidos al lanzar. No implementa `Debug` ni `Serialize`
/// para reducir el riesgo de persistirlos o incluirlos accidentalmente en logs.
#[derive(Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct LaunchValues(HashMap<String, String>);

impl LaunchValues {
    pub fn new(values: HashMap<String, String>) -> Self {
        Self(values)
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    pub fn redaction_values(&self) -> Vec<String> {
        let mut values: Vec<_> = self
            .0
            .values()
            .filter(|value| !value.is_empty())
            .cloned()
            .collect();
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values.dedup();
        values
    }
}

impl From<HashMap<String, String>> for LaunchValues {
    fn from(values: HashMap<String, String>) -> Self {
        Self::new(values)
    }
}

pub fn extract_launch_fields(args: &[String]) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut seen = HashSet::new();

    for (index, argument) in args.iter().enumerate() {
        for key in template_fields(argument, index)? {
            if seen.insert(key.clone()) {
                if fields.len() >= MAX_LAUNCH_FIELDS {
                    return Err(format!(
                        "Los argumentos no pueden usar más de {MAX_LAUNCH_FIELDS} campos"
                    ));
                }
                fields.push(key);
            }
        }
    }

    Ok(fields)
}

pub fn render_launch_args(args: &[String], values: &LaunchValues) -> Result<Vec<String>, String> {
    validate_argument_list("de lanzamiento", args)?;
    let required = extract_launch_fields(args)?;
    validate_values(&required, values)?;

    let mut rendered = Vec::with_capacity(args.len());
    let mut total_length = 0;
    for (index, argument) in args.iter().enumerate() {
        let value = render_argument(argument, index, values)?;
        let length = value.chars().count();
        if length > MAX_LAUNCH_ARGUMENT_LENGTH {
            return Err(format!(
                "El argumento de lanzamiento {} supera {MAX_LAUNCH_ARGUMENT_LENGTH} caracteres tras resolver sus campos",
                index + 1
            ));
        }
        total_length += length;
        if total_length > MAX_LAUNCH_TOTAL_LENGTH {
            return Err(format!(
                "Los argumentos de lanzamiento superan {MAX_LAUNCH_TOTAL_LENGTH} caracteres en total"
            ));
        }
        rendered.push(value);
    }
    Ok(rendered)
}

fn validate_argument_list(label: &str, args: &[String]) -> Result<(), String> {
    if args.len() > MAX_LAUNCH_ARGUMENTS {
        return Err(format!(
            "Los argumentos {label} no pueden superar {MAX_LAUNCH_ARGUMENTS} elementos"
        ));
    }

    let mut total_length = 0;
    for (index, argument) in args.iter().enumerate() {
        if argument.trim().is_empty() {
            return Err(format!(
                "El argumento {label} {} no puede estar vacío",
                index + 1
            ));
        }
        if contains_control_character(argument) {
            return Err(format!(
                "El argumento {label} {} contiene un carácter de control",
                index + 1
            ));
        }
        let length = argument.chars().count();
        if length > MAX_LAUNCH_ARGUMENT_LENGTH {
            return Err(format!(
                "El argumento {label} {} no puede superar {MAX_LAUNCH_ARGUMENT_LENGTH} caracteres",
                index + 1
            ));
        }
        total_length += length;
        if total_length > MAX_LAUNCH_TOTAL_LENGTH {
            return Err(format!(
                "Los argumentos {label} no pueden superar {MAX_LAUNCH_TOTAL_LENGTH} caracteres en total"
            ));
        }
        template_fields(argument, index)?;
    }

    extract_launch_fields(args)?;
    Ok(())
}

fn validate_values(required: &[String], values: &LaunchValues) -> Result<(), String> {
    if values.len() > MAX_LAUNCH_FIELDS {
        return Err(format!(
            "No se pueden proporcionar más de {MAX_LAUNCH_FIELDS} valores de lanzamiento"
        ));
    }

    let required: HashSet<&str> = required.iter().map(String::as_str).collect();
    for (key, value) in values.iter() {
        validate_field_key(key)?;
        if !required.contains(key) {
            return Err(format!(
                "El campo de lanzamiento '${{{key}}}' no es esperado"
            ));
        }
        if value.is_empty() {
            return Err(format!(
                "El valor del campo de lanzamiento '${{{key}}}' no puede estar vacío"
            ));
        }
        if contains_control_character(value) {
            return Err(format!(
                "El valor del campo de lanzamiento '${{{key}}}' contiene un carácter de control"
            ));
        }
        if value.chars().count() > MAX_LAUNCH_VALUE_LENGTH {
            return Err(format!(
                "El valor del campo de lanzamiento '${{{key}}}' supera {MAX_LAUNCH_VALUE_LENGTH} caracteres"
            ));
        }
    }

    for key in &required {
        if values.get(key).is_none() {
            return Err(format!(
                "Falta el valor del campo de lanzamiento '${{{key}}}'"
            ));
        }
    }
    Ok(())
}

fn contains_control_character(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn template_fields(argument: &str, index: usize) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = argument[cursor..].find("${") {
        let start = cursor + relative_start;
        let key_start = start + 2;
        let Some(relative_end) = argument[key_start..].find('}') else {
            return Err(format!(
                "El argumento {} contiene un campo de lanzamiento sin cerrar",
                index + 1
            ));
        };
        let end = key_start + relative_end;
        let key = &argument[key_start..end];
        validate_field_key(key)?;
        fields.push(key.to_string());
        cursor = end + 1;
    }

    Ok(fields)
}

fn validate_field_key(key: &str) -> Result<(), String> {
    let length = key.len();
    let mut chars = key.chars();
    let valid_start = chars.next().is_some_and(|ch| ch.is_ascii_alphabetic());
    let valid_rest = chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if !valid_start || !valid_rest || length > MAX_LAUNCH_FIELD_KEY_LENGTH {
        return Err(format!(
            "La clave de lanzamiento '${{{key}}}' no es válida; usa una letra inicial y hasta {MAX_LAUNCH_FIELD_KEY_LENGTH} letras, números, '_' o '-'"
        ));
    }
    Ok(())
}

fn render_argument(argument: &str, index: usize, values: &LaunchValues) -> Result<String, String> {
    let mut rendered = String::with_capacity(argument.len());
    let mut cursor = 0;

    while let Some(relative_start) = argument[cursor..].find("${") {
        let start = cursor + relative_start;
        rendered.push_str(&argument[cursor..start]);
        let key_start = start + 2;
        let Some(relative_end) = argument[key_start..].find('}') else {
            return Err(format!(
                "El argumento {} contiene un campo de lanzamiento sin cerrar",
                index + 1
            ));
        };
        let end = key_start + relative_end;
        let key = &argument[key_start..end];
        validate_field_key(key)?;
        let value = values
            .get(key)
            .ok_or_else(|| format!("Falta el valor del campo de lanzamiento '${{{key}}}'"))?;
        rendered.push_str(value);
        cursor = end + 1;
    }
    rendered.push_str(&argument[cursor..]);
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(entries: &[(&str, &str)]) -> LaunchValues {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<HashMap<_, _>>()
            .into()
    }

    #[test]
    fn extracts_unique_fields_in_argument_order() {
        let args = vec![
            "-t:${password}".to_string(),
            "${userId}".to_string(),
            "again-${password}".to_string(),
        ];
        assert_eq!(
            extract_launch_fields(&args).unwrap(),
            vec!["password", "userId"]
        );
    }

    #[test]
    fn renders_sakura_arguments_without_splitting_or_shell_expansion() {
        let config = LaunchConfig {
            game_args: vec![
                "-t:${password}".into(),
                "${userId}".into(),
                "40.160.233.207".into(),
                "-1rag1".into(),
                "literal $() and spaces".into(),
            ],
            ..Default::default()
        };
        let rendered = config
            .render_args(&values(&[("password", "s3cr et"), ("userId", "alice")]))
            .unwrap();
        assert_eq!(
            rendered,
            vec![
                "-t:s3cr et",
                "alice",
                "40.160.233.207",
                "-1rag1",
                "literal $() and spaces"
            ]
        );
    }

    #[test]
    fn uses_only_the_selected_strategy_arguments() {
        let config = LaunchConfig {
            strategy: LaunchStrategy::Patcher,
            game_args: vec!["${gamePassword}".into()],
            patcher_args: vec!["--channel".into(), "stable".into()],
            ..Default::default()
        };
        assert!(extract_launch_fields(config.active_args())
            .unwrap()
            .is_empty());
        assert_eq!(
            config.render_args(&LaunchValues::default()).unwrap(),
            vec!["--channel", "stable"]
        );
    }

    #[test]
    fn rejects_missing_unknown_invalid_and_nul_values() {
        let config = LaunchConfig {
            game_args: vec!["${password}".into()],
            ..Default::default()
        };
        assert!(config.render_args(&LaunchValues::default()).is_err());
        assert!(config
            .render_args(&values(&[("password", "ok"), ("other", "value")]))
            .is_err());
        assert!(config
            .render_args(&values(&[("bad key", "value")]))
            .is_err());
        let error = config
            .render_args(&values(&[("password", "hidden\0value")]))
            .unwrap_err();
        assert!(!error.contains("hidden"));
    }

    #[test]
    fn rejects_malformed_templates_and_limits() {
        let malformed = vec!["${password".to_string()];
        assert!(extract_launch_fields(&malformed).is_err());

        let invalid_key = vec!["${1password}".to_string()];
        assert!(extract_launch_fields(&invalid_key).is_err());

        let too_many = vec!["arg".to_string(); MAX_LAUNCH_ARGUMENTS + 1];
        assert!(validate_argument_list("del juego", &too_many).is_err());
    }

    #[test]
    fn rejects_empty_and_multiline_arguments_or_values() {
        let empty_arg = LaunchConfig {
            game_args: vec![String::new()],
            ..Default::default()
        };
        assert!(empty_arg.validate().is_err());

        let config = LaunchConfig {
            game_args: vec!["${password}".into()],
            ..Default::default()
        };
        assert!(config
            .render_args(&values(&[("password", "line1\nline2")]))
            .is_err());
        assert!(config.render_args(&values(&[("password", "")])).is_err());
    }

    #[test]
    fn serializes_the_manual_webview2_override_with_the_shared_contract_name() {
        let config = LaunchConfig {
            require_webview2: true,
            ..Default::default()
        };
        let json = serde_json::to_value(config).unwrap();
        assert_eq!(json.get("requireWebview2"), Some(&serde_json::json!(true)));
    }
}
