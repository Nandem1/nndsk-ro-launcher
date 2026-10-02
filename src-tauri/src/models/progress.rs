use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub step: String,
    pub percent: u32,
    pub operation_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_identity_uses_the_frontend_contract() {
        let event = ProgressEvent {
            step: "Ready".into(),
            percent: 100,
            operation_id: Some("current".into()),
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "step": "Ready", "percent": 100, "operationId": "current"
            })
        );
    }
}
