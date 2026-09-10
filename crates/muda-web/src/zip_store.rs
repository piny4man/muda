//! Minimal ZIP (store method) so batch download works in WASM without extra crates.

pub fn zip_store(entries: &[(String, &[u8])]) -> Vec<u8> {
    let mut local = Vec::new();
    let mut central = Vec::new();
    let mut count: u16 = 0;

    for (name, data) in entries {
        let name_bytes = name.as_bytes();
        let crc = crc32(data);
        let size = data.len() as u32;
        let offset = local.len() as u32;
        let name_len = name_bytes.len() as u16;

        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
        local.extend_from_slice(&0u16.to_le_bytes()); // store
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&name_len.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_bytes);
        local.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);

        count = count.saturating_add(1);
    }

    let cd_offset = local.len() as u32;
    let cd_size = central.len() as u32;
    local.extend_from_slice(&central);

    local.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(&count.to_le_bytes());
    local.extend_from_slice(&count.to_le_bytes());
    local.extend_from_slice(&cd_size.to_le_bytes());
    local.extend_from_slice(&cd_offset.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_has_local_and_eocd() {
        let bytes = zip_store(&[("a.cleaned.jpg".into(), b"hello".as_slice())]);
        assert_eq!(&bytes[..4], &0x0403_4b50u32.to_le_bytes());
        assert!(bytes.windows(4).any(|w| w == 0x0605_4b50u32.to_le_bytes()));
        assert!(bytes
            .windows(b"a.cleaned.jpg".len())
            .any(|w| w == b"a.cleaned.jpg"));
        assert!(bytes.windows(5).any(|w| w == b"hello"));
    }
}
