//! IEEE CRC-32 (zlib-compatible) for mod-file verification (STR ModPolicy).
//!
//! Both the server's `--list-mod-crc` tool and (later) the bridge's load-order
//! reporting must hash the raw file bytes identically — this is that one
//! implementation, so the two sides can never disagree.

/// IEEE CRC-32 of a byte slice (polynomial 0xEDB88320, zlib/`crc32`-compatible).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

/// Streaming CRC-32 over a reader (files too big for one buffer — 276MB ESMs).
pub fn file_crc32(path: &std::path::Path) -> std::io::Result<u32> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut crc = 0xFFFF_FFFFu32;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
    }
    Ok(!crc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_standard_vector() {
        // The canonical CRC-32 check value: "123456789" → 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn test_file_crc32_matches_in_memory() {
        let dir = std::env::temp_dir();
        let path = dir.join("ashfall_crc_test.bin");
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &data).unwrap();
        assert_eq!(file_crc32(&path).unwrap(), crc32(&data), "streamed == in-memory");
        std::fs::remove_file(&path).ok();
    }
}
