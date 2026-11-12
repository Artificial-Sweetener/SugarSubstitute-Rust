// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual decoder behavior with bounded source data and external read failures.
use super::*;
struct Source {
    data: Vec<u8>,
    fail: bool,
    maximum: usize,
}
impl ReadAt for Source {
    type Error = std::io::Error;
    fn length(&self) -> u64 {
        self.data.len() as u64
    }
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error> {
        self.maximum = self.maximum.max(output.len());
        if self.fail {
            return Err(std::io::Error::from_raw_os_error(57));
        }
        let slice = self
            .data
            .get(offset as usize..offset as usize + output.len())
            .ok_or_else(|| std::io::Error::other("fixture range"))?;
        output.copy_from_slice(slice);
        Ok(())
    }
}
fn source(data: Vec<u8>) -> Source {
    Source { data, fail: false, maximum: 0 }
}
fn write_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn write_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn pe() -> Vec<u8> {
    let mut data = vec![0; 1536];
    data[..2].copy_from_slice(b"MZ");
    write_u32(&mut data, 60, 128);
    data[128..132].copy_from_slice(b"PE\0\0");
    write_u16(&mut data, 132, 0x8664);
    write_u16(&mut data, 134, 1);
    write_u32(&mut data, 136, 19);
    write_u16(&mut data, 148, 240);
    write_u16(&mut data, 150, 2);
    write_u16(&mut data, 152, 0x20b);
    write_u32(&mut data, 208, 0x2000);
    write_u32(&mut data, 212, 512);
    write_u32(&mut data, 216, 29);
    write_u32(&mut data, 260, 16);
    write_u32(&mut data, 312, 0x1100);
    write_u32(&mut data, 316, 28);
    write_u32(&mut data, 400, 1024);
    write_u32(&mut data, 404, 0x1000);
    write_u32(&mut data, 408, 1024);
    write_u32(&mut data, 412, 512);
    write_u32(&mut data, 780, 2);
    write_u32(&mut data, 784, 30);
    write_u32(&mut data, 788, 0x1200);
    write_u32(&mut data, 792, 1024);
    data[1024..1028].copy_from_slice(b"RSDS");
    data[1028..1044].fill(3);
    write_u32(&mut data, 1044, 1);
    data[1048..1054].copy_from_slice(b"a.pdb\0");
    data
}
fn pdb() -> Vec<u8> {
    let mut data = vec![0; 512 * 8];
    data[..32].copy_from_slice(b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0");
    write_u32(&mut data, 32, 512);
    write_u32(&mut data, 36, 1);
    write_u32(&mut data, 40, 8);
    write_u32(&mut data, 44, 20);
    write_u32(&mut data, 52, 3);
    write_u32(&mut data, 512 * 3, 4);
    for (offset, value) in [(0, 3), (4, u32::MAX), (8, 28), (12, 0), (16, 7)] {
        write_u32(&mut data, 512 * 4 + offset, value);
    }
    write_u32(&mut data, 512 * 7, 20000404);
    write_u32(&mut data, 512 * 7 + 8, 1);
    data[512 * 7 + 12..512 * 7 + 28].fill(3);
    data
}
#[test]
fn exact_amd64_rsds_and_actual_pdb_info_match_without_whole_file_allocation()
-> Result<(), Box<dyn std::error::Error>> {
    let mut image = source(pe());
    let actual = read_pe(&mut image).map_err(|cause| format!("{cause:?}"))?;
    assert_eq!(
        actual,
        PeIdentity {
            kind: PeKind::Executable,
            image_size: 0x2000,
            timestamp: 19,
            checksum: 29,
            pdb: PdbIdentity { guid: [3; 16], age: 1 }
        }
    );
    let mut symbols = source(pdb());
    assert_eq!(read_pdb(&mut symbols).map_err(|cause| format!("{cause:?}"))?, actual.pdb);
    assert!(image.maximum <= 240);
    assert!(symbols.maximum <= 56);
    write_u16(&mut image.data, 150, 0x2002);
    assert_eq!(read_pe(&mut image).map_err(|cause| format!("{cause:?}"))?.kind, PeKind::Library);
    Ok(())
}
#[test]
fn pe_unknown_architecture_schemas_ranges_and_ambiguous_debug_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    for (offset, value) in [
        (60, u32::MAX),
        (208, 0),
        (212, 1),
        (260, 6),
        (316, 0),
        (316, 29),
        (316, 28 * 17),
        (404, u32::MAX),
        (408, u32::MAX),
        (784, 24),
        (784, 4097),
        (788, 0x1110),
        (792, u32::MAX),
        (1044, 0),
    ] {
        let mut data = pe();
        write_u32(&mut data, offset, value);
        assert!(read_pe(&mut source(data)).is_err(), "{offset}:{value}");
    }
    for (offset, value) in [(132, 0x14c), (134, 0), (134, 97), (148, 239), (152, 0x10b)] {
        let mut data = pe();
        write_u16(&mut data, offset, value);
        assert!(read_pe(&mut source(data)).is_err(), "{offset}:{value}");
    }
    let mut data = pe();
    write_u32(&mut data, 316, 56);
    let debug = data[768..796].to_vec();
    data[796..824].copy_from_slice(&debug);
    assert!(read_pe(&mut source(data)).is_err());
    let mut data = pe();
    data[1028..1044].fill(0);
    assert!(read_pe(&mut source(data)).is_err());
    let mut data = pe();
    data[1050] = 0;
    assert!(read_pe(&mut source(data)).is_err());
    Ok(())
}
#[test]
fn pdb_invalid_superblock_directory_info_and_reserved_pages_reject()
-> Result<(), Box<dyn std::error::Error>> {
    for (offset, value) in [
        (32, 513),
        (36, 3),
        (40, 7),
        (44, 15),
        (44, u32::MAX),
        (52, 0),
        (52, 1),
        (52, 2),
        (52, 8),
        (512 * 3, 3),
        (512 * 3, 8),
        (512 * 4, 1),
        (512 * 4, 65537),
        (512 * 4 + 8, 27),
        (512 * 4 + 8, u32::MAX),
        (512 * 4 + 4, 512 * 8),
        (512 * 4 + 16, 1),
        (512 * 4 + 16, 3),
        (512 * 4 + 16, 4),
        (512 * 4 + 16, 8),
        (512 * 7, 0),
        (512 * 7 + 8, 0),
    ] {
        let mut data = pdb();
        write_u32(&mut data, offset, value);
        assert!(read_pdb(&mut source(data)).is_err(), "{offset}:{value}");
    }
    let mut data = pdb();
    data[512 * 7 + 12..512 * 7 + 28].fill(0);
    assert!(read_pdb(&mut source(data)).is_err());
    let mut data = pdb();
    data[0] = 0;
    assert!(read_pdb(&mut source(data)).is_err());
    Ok(())
}
#[test]
fn every_required_header_truncation_and_original_native_read_failure_is_observable()
-> Result<(), Box<dyn std::error::Error>> {
    for limit in 0..1054 {
        let mut data = pe();
        data.truncate(limit);
        assert!(read_pe(&mut source(data)).is_err(), "PE {limit}");
    }
    for limit in 0..4096 {
        let mut data = pdb();
        data.truncate(limit);
        assert!(read_pdb(&mut source(data)).is_err(), "PDB {limit}");
    }
    for mut data in [source(pe()), source(pdb())] {
        data.fail = true;
        let result = if data.data.len() == 4096 {
            read_pdb(&mut data).map(|_| ())
        } else {
            read_pe(&mut data).map(|_| ())
        };
        match result.err().ok_or("expected original read failure")? {
            ImageError::Read(error) => assert_eq!(error.raw_os_error(), Some(57)),
            other => return Err(format!("Read cause flattened: {other:?}").into()),
        }
    }
    Ok(())
}
