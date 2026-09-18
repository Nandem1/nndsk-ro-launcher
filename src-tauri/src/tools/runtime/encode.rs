use sha2::{Digest, Sha256};

pub(crate) const PREFIX_FINGERPRINT_DOMAIN: &[u8] = b"ro-launcher/prefix-fingerprint/v1\0";
pub(crate) const RUNTIME_FINGERPRINT_DOMAIN: &[u8] = b"ro-launcher/runtime-fingerprint/v1\0";
pub(crate) const SERVER_PATH_DOMAIN: &[u8] = b"ro-launcher/server-path/v1\0";
pub(crate) const RUNNER_LOCATION_DOMAIN: &[u8] = b"ro-launcher/runner-location/v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CanonicalValue {
    Null,
    Bool(bool),
    U64(u64),
    String(String),
    Bytes(Vec<u8>),
    Sequence(Vec<CanonicalValue>),
    Record(Vec<(String, CanonicalValue)>),
    Enum {
        variant: String,
        payload: Box<CanonicalValue>,
    },
}

impl CanonicalValue {
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Self::Null => out.push(0x00),
            Self::Bool(false) => out.push(0x01),
            Self::Bool(true) => out.push(0x02),
            Self::U64(value) => {
                out.push(0x03);
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::String(value) => {
                out.push(0x04);
                let bytes = value.as_bytes();
                out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                out.extend_from_slice(bytes);
            }
            Self::Bytes(value) => {
                out.push(0x05);
                out.extend_from_slice(&(value.len() as u32).to_be_bytes());
                out.extend_from_slice(value);
            }
            Self::Sequence(values) => {
                out.push(0x06);
                out.extend_from_slice(&(values.len() as u32).to_be_bytes());
                for value in values {
                    value.encode(out);
                }
            }
            Self::Record(fields) => {
                out.push(0x07);
                out.extend_from_slice(&(fields.len() as u32).to_be_bytes());
                for (field_id, value) in fields {
                    Self::String(field_id.clone()).encode(out);
                    value.encode(out);
                }
            }
            Self::Enum { variant, payload } => {
                out.push(0x08);
                Self::String(variant.clone()).encode(out);
                payload.as_ref().encode(out);
            }
        }
    }

    pub(crate) fn record(fields: Vec<(String, CanonicalValue)>) -> Self {
        let mut sorted = fields;
        sorted.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        Self::Record(sorted)
    }

    pub(crate) fn enum_variant(variant: impl Into<String>, payload: CanonicalValue) -> Self {
        Self::Enum {
            variant: variant.into(),
            payload: Box::new(payload),
        }
    }

    pub(crate) fn empty_record() -> Self {
        Self::Record(Vec::new())
    }
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

pub(crate) fn fingerprint_digest(domain: &[u8], root: &CanonicalValue) -> [u8; 32] {
    let mut payload = Vec::new();
    payload.extend_from_slice(domain);
    root.encode(&mut payload);
    let digest = Sha256::digest(&payload);
    digest.into()
}

pub(crate) fn server_path_token16(server_id: &str) -> String {
    let mut payload = Vec::new();
    payload.extend_from_slice(SERVER_PATH_DOMAIN);
    let bytes = server_id.as_bytes();
    payload.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    payload.extend_from_slice(bytes);
    sha256_hex(&payload)[..16].to_string()
}

pub(crate) fn external_runner_location_token(path_bytes: &[u8]) -> [u8; 32] {
    let mut payload = Vec::new();
    payload.extend_from_slice(RUNNER_LOCATION_DOMAIN);
    payload.extend_from_slice(&(path_bytes.len() as u32).to_be_bytes());
    payload.extend_from_slice(path_bytes);
    Sha256::digest(&payload).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_record_bytes() {
        let record = CanonicalValue::record(vec![
            ("count".to_string(), CanonicalValue::U64(1)),
            ("flag".to_string(), CanonicalValue::Bool(true)),
        ]);
        let mut encoded = Vec::new();
        record.encode(&mut encoded);
        assert_eq!(
            sha256_hex(&encoded),
            "855bf92f11437f37d149d93f865ac9dff6354980f5f546a774ca88d95d03f791"
        );
        let mut hex = String::new();
        for byte in encoded {
            hex.push_str(&format!("{:02x}", byte));
        }
        assert_eq!(
            hex,
            "07000000020400000005636f756e740300000000000000010400000004666c616702"
        );
    }

    #[test]
    fn golden_server_path_token() {
        assert_eq!(server_path_token16("fixture-server"), "d8e0b61318835001");
    }

    #[test]
    fn golden_external_location_token() {
        let path = b"/opt/wine/bin/wine";
        let mut payload = Vec::new();
        payload.extend_from_slice(RUNNER_LOCATION_DOMAIN);
        payload.extend_from_slice(&(path.len() as u32).to_be_bytes());
        payload.extend_from_slice(path);
        assert_eq!(
            sha256_hex(&payload),
            "991911a552070c3379dcb3f09f1389363b49e2fc09f5ba75af38fae58d453301"
        );
    }
}
