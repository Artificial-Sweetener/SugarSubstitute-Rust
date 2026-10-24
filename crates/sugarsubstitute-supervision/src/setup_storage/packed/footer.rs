// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Locate the authenticated payload before an optional Windows certificate table.
//! PE certificate offsets and eight-byte alignment follow Microsoft's PE format:
//! <https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#the-attribute-certificate-table-image-only>.
use super::{MAGIC, invalid};
use std::io::{self, Read, Seek, SeekFrom};

/// Certificate records locate the payload; they never substitute for its compiled SHA-256 trust.
pub(super) fn end(file: &mut (impl Read + Seek)) -> io::Result<u64> {
    let length = file.seek(SeekFrom::End(0))?;
    if length < 24 {
        return Err(invalid("Installer payload footer is absent"));
    }
    if &read::<8>(file, length - 8)? == MAGIC {
        return Ok(length);
    }
    let dos = read::<64>(file, 0)?;
    if &dos[..2] != b"MZ" {
        return Err(invalid("Installer image header is invalid"));
    }
    let pe = u64::from(u32::from_le_bytes(dos[60..64].try_into().map_err(io::Error::other)?));
    if !(64..=1024 * 1024).contains(&pe) {
        return Err(invalid("Installer image header exceeds its bound"));
    }
    let header = read::<24>(file, pe)?;
    if &header[..4] != b"PE\0\0" {
        return Err(invalid("Installer PE header is invalid"));
    }
    let optional_size = u64::from(u16::from_le_bytes([header[20], header[21]]));
    let optional = pe + 24;
    let (count_offset, certificate_offset) = match u16::from_le_bytes(read(file, optional)?) {
        0x10b => (92, 128),
        0x20b => (108, 144),
        _ => return Err(invalid("Unsupported installer image header")),
    };
    if optional_size < certificate_offset + 8
        || optional_size > 4096
        || u32::from_le_bytes(read(file, optional + count_offset)?) < 5
    {
        return Err(invalid("Installer certificate directory is absent"));
    }
    let certificate = read::<8>(file, optional + certificate_offset)?;
    let start =
        u64::from(u32::from_le_bytes(certificate[..4].try_into().map_err(io::Error::other)?));
    let size =
        u64::from(u32::from_le_bytes(certificate[4..].try_into().map_err(io::Error::other)?));
    if start < optional + optional_size
        || start % 8 != 0
        || !(8..=16 * 1024 * 1024).contains(&size)
        || start + size != length
    {
        return Err(invalid("Installer certificate table has invalid bounds"));
    }
    let mut position = start;
    let mut count = 0;
    while position < length {
        count += 1;
        if count > 1024 {
            return Err(invalid("Installer certificate inventory exceeds its bound"));
        }
        let entry = u64::from(u32::from_le_bytes(read(file, position)?));
        if entry < 8 {
            return Err(invalid("Installer certificate entry is truncated"));
        }
        position += (entry + 7) & !7;
        if position > length {
            return Err(invalid("Installer certificate entry exceeds its table"));
        }
    }
    let tail = read::<31>(file, start - 31)?;
    for padding in 0..8 {
        let end = tail.len() - padding;
        if tail[end..].iter().all(|byte| *byte == 0) && &tail[end - 8..end] == MAGIC {
            return Ok(start - padding as u64);
        }
    }
    Err(invalid("Installer payload footer is invalid"))
}

fn read<const N: usize>(file: &mut (impl Read + Seek), offset: u64) -> io::Result<[u8; N]> {
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = [0; N];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(pe32: bool, padding: usize) -> Vec<u8> {
        let mut bytes = vec![0; 512 + 24 - padding];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&128_u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[148..150].copy_from_slice(&240_u16.to_le_bytes());
        bytes[152..154].copy_from_slice(&(if pe32 { 0x10b_u16 } else { 0x20b }).to_le_bytes());
        let (count, certificate) = if pe32 { (244, 280) } else { (260, 296) };
        bytes[count..count + 4].copy_from_slice(&16_u32.to_le_bytes());
        bytes[certificate..certificate + 4].copy_from_slice(&536_u32.to_le_bytes());
        bytes[certificate + 4..certificate + 8].copy_from_slice(&16_u32.to_le_bytes());
        let end = bytes.len();
        bytes[end - 8..].copy_from_slice(MAGIC);
        bytes.resize(536, 0);
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&[0, 2, 2, 0, 1, 2, 3, 4, 0, 0, 0, 0]);
        bytes
    }

    #[test]
    fn pe32_and_pe64_certificate_alignment_preserves_every_footer_position() -> io::Result<()> {
        for pe32 in [false, true] {
            for padding in 0..8 {
                let bytes = image(pe32, padding);
                assert_eq!(end(&mut io::Cursor::new(bytes))?, 536 - padding as u64);
            }
        }
        Ok(())
    }

    #[test]
    fn truncated_certificates_and_nonzero_alignment_are_rejected() {
        let valid = image(false, 3);
        assert!(end(&mut io::Cursor::new(&valid[..valid.len() - 1])).is_err());
        let mut padding = valid.clone();
        padding[535] = 1;
        assert!(end(&mut io::Cursor::new(padding)).is_err());
        let mut entry = valid.clone();
        entry[536..540].copy_from_slice(&32_u32.to_le_bytes());
        assert!(end(&mut io::Cursor::new(entry)).is_err());
        let mut table = valid;
        table[296..300].copy_from_slice(&535_u32.to_le_bytes());
        assert!(end(&mut io::Cursor::new(table)).is_err());
    }
}
