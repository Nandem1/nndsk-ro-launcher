//! Bounded PE header identity shared by client diagnostics and artifact qualification.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PeArchitecture {
    I386,
    Amd64,
}

#[derive(Debug)]
pub(crate) struct PeHeaders {
    pub coff: usize,
    pub optional: usize,
    pub optional_size: usize,
    pub machine: u16,
    pub magic: u16,
}

pub(crate) fn pe_headers(bytes: &[u8]) -> Option<PeHeaders> {
    if bytes.get(..2)? != b"MZ" {
        return None;
    }
    let pe = usize::try_from(read_u32(bytes, 0x3c)?).ok()?;
    if pe < 0x40 || bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let coff = pe.checked_add(4)?;
    let optional = coff.checked_add(20)?;
    bytes.get(coff..optional)?;
    let optional_size = usize::from(read_u16(bytes, coff.checked_add(16)?)?);
    let header = bytes.get(optional..optional.checked_add(optional_size)?)?;
    Some(PeHeaders {
        coff,
        optional,
        optional_size,
        machine: read_u16(bytes, coff)?,
        magic: read_u16(header, 0)?,
    })
}

/// Unknown machines and contradictory machine/class pairs never qualify. Diagnostics may
/// separately retain their historical optional-header fallback using the raw headers.
pub(crate) fn pe_architecture(bytes: &[u8]) -> Option<PeArchitecture> {
    let header = pe_headers(bytes)?;
    match (header.machine, header.magic) {
        (0x014c, 0x010b) if header.optional_size >= 96 => Some(PeArchitecture::I386),
        (0x8664, 0x020b) if header.optional_size >= 112 => Some(PeArchitecture::Amd64),
        _ => None,
    }
}

pub(crate) fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

pub(crate) fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub(crate) fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}

#[cfg(test)]
pub(crate) fn test_pe(machine: u16, magic: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x188];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&0xf0u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&magic.to_le_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pe_identity_requires_machine_and_class() {
        for (machine, magic, expected) in [
            (0x014c, 0x010b, Some(PeArchitecture::I386)),
            (0x8664, 0x020b, Some(PeArchitecture::Amd64)),
            (0x014c, 0x020b, None),
            (0x8664, 0x010b, None),
            (0xffff, 0x010b, None),
            (0xffff, 0x020b, None),
            (0x014c, 0, None),
        ] {
            assert_eq!(pe_architecture(&test_pe(machine, magic)), expected);
        }
    }

    #[test]
    fn pe_identity_rejects_malformed_and_every_truncated_header_without_panicking() {
        for (machine, magic) in [(0x014c, 0x010b), (0x8664, 0x020b)] {
            let valid = test_pe(machine, magic);
            for end in 0..valid.len() {
                assert_eq!(pe_architecture(&valid[..end]), None, "length={end}");
            }
            for (offset, value) in [(0, 0), (0x80, 0), (0x94, 0), (0x95, 0xff)] {
                let mut bytes = valid.clone();
                bytes[offset] = value;
                assert_eq!(pe_architecture(&bytes), None, "offset={offset}");
            }
            for offset in [0, 0x3f, 0x189, u32::MAX] {
                let mut bytes = valid.clone();
                bytes[0x3c..0x40].copy_from_slice(&offset.to_le_bytes());
                assert_eq!(pe_architecture(&bytes), None);
            }
            let mut short_optional = valid;
            short_optional[0x94..0x96].copy_from_slice(&2u16.to_le_bytes());
            assert_eq!(pe_architecture(&short_optional), None);
        }
        assert_eq!(read_u16(&[], usize::MAX), None);
        assert_eq!(read_u32(&[], usize::MAX), None);
        assert_eq!(read_u64(&[], usize::MAX), None);
    }
}
