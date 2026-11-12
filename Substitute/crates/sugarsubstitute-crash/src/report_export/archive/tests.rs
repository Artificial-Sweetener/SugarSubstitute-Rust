// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify bounded standard ZIP metadata and exact complete Unicode report bytes.
use super::*;
use std::error::Error;
struct Bytes(Vec<u8>);
impl Sink for Bytes {
    fn put(&mut self, bytes: &[u8]) -> Result<(), E> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn position(&self) -> u64 {
        self.0.len() as u64
    }
}
#[test]
fn report_export_crc_matches_known_vector_and_chunked_input() {
    let mut whole = Crc::new();
    whole.update(b"123456789");
    let mut chunked = Crc::new();
    chunked.update(b"1234");
    chunked.update(b"56789");
    assert_eq!(whole.finish(), 0xcbf4_3926);
    assert_eq!(chunked.finish(), 0xcbf4_3926);
}
#[test]
fn report_export_zip_has_exact_names_complete_bytes_descriptors_and_directory()
-> Result<(), Box<dyn Error>> {
    let mut sink = Bytes(Vec::new());
    let contents =
        ["糖😀\ncomplete canonical report\n".as_bytes(), b"actual-boundary-dump-bytes".as_slice()];
    let mut entries = [Entry { offset: 0, size: 0, crc: 0 }; 2];
    for ((name, content), entry) in
        [TEXT_NAME, DUMP_NAME].into_iter().zip(contents).zip(&mut entries)
    {
        entry.offset = header(&mut sink, name)?;
        entry.size = content.len() as u32;
        let mut crc = Crc::new();
        crc.update(content);
        entry.crc = crc.finish();
        sink.put(content)?;
        descriptor(&mut sink, *entry)?;
    }
    finish(&mut sink, entries)?;
    assert_eq!(sink.position(), bundle_bytes(contents[0].len() as u64, contents[1].len() as u64)?);
    for ((name, content), entry) in [TEXT_NAME, DUMP_NAME].into_iter().zip(contents).zip(entries) {
        let start = entry.offset as usize;
        assert_eq!(&sink.0[start..start + 4], &0x0403_4b50_u32.to_le_bytes());
        assert_eq!(&sink.0[start + 30..start + 30 + name.len()], name);
        let data = start + 30 + name.len();
        assert_eq!(&sink.0[data..data + content.len()], content);
        assert_eq!(
            &sink.0[data + content.len()..data + content.len() + 4],
            &0x0807_4b50_u32.to_le_bytes()
        );
    }
    assert_eq!(&sink.0[sink.0.len() - 22..sink.0.len() - 18], &0x0605_4b50_u32.to_le_bytes());
    assert_eq!(&sink.0[sink.0.len() - 12..sink.0.len() - 10], &2_u16.to_le_bytes());
    Ok(())
}
#[test]
fn report_export_zip_propagates_sink_failure_and_rejects_zip64_size() {
    struct Rejected;
    impl Sink for Rejected {
        fn put(&mut self, _: &[u8]) -> Result<(), E> {
            Err(E::Rejected(C::Native))
        }
        fn position(&self) -> u64 {
            0
        }
    }
    assert!(header(&mut Rejected, TEXT_NAME).is_err());
    assert!(count(u64::from(u32::MAX) + 1).is_err());
}

#[test]
fn report_export_zip_complete_result_bound_includes_fixed_envelope() -> Result<(), E> {
    assert_eq!(bundle_bytes(262_144, 268_173_066)?, 268_435_456);
    assert!(bundle_bytes(262_144, 268_173_067).is_err());
    assert!(bundle_bytes(1, u64::MAX).is_err());
    Ok(())
}
