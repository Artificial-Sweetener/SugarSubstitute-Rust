// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish the validated Crashpad annotation carrier from the captured fault image.
use super::*;
/// Supply two real-layout module records with independent CodeView identities.
fn split_images(annotation_index: u32) -> Bytes {
    let mut bytes = input();
    let mut fault = [0; 108];
    fault.copy_from_slice(&bytes.0[260..368]);
    bytes.0.resize(768, 0);
    bytes.0[256..512].fill(0);
    put(&mut bytes.0, 48, 220);
    put(&mut bytes.0, 256, 2);
    for index in 0..2 {
        let start = 260 + index * 108;
        bytes.0[start..start + 108].copy_from_slice(&fault);
        let annotation = index == annotation_index as usize;
        let base = if annotation { 0x3000_u64 } else { 0x1000 };
        bytes.0[start..start + 8].copy_from_slice(&base.to_le_bytes());
        put(&mut bytes.0, start + 80, if annotation { 632 } else { 600 });
    }
    for (start, guid) in [(600, 3), (632, 8)] {
        bytes.0[start..start + 4].copy_from_slice(b"RSDS");
        bytes.0[start + 4..start + 20].fill(guid);
        put(&mut bytes.0, start + 20, 1);
    }
    bytes
}
#[test]
fn annotation_carrier_does_not_choose_fault_image_in_either_module_order()
-> Result<(), Box<dyn std::error::Error>> {
    for annotation in [0, 1] {
        let mut bytes = split_images(annotation);
        let frame = extract(&mut bytes, 768, Some(annotation), failure())
            .map_err(|cause| std::io::Error::other(format!("actual frame: {cause:?}")))?;
        assert_eq!((frame.address(), frame.image_base(), frame.image_size()), (0x1008, 0x1000, 64));
        assert_eq!(frame.pdb(), PdbIdentity::new([3; 16], 1)?);
    }
    let mut many = split_images(1);
    many.0.resize(4096, 0);
    let mut fault = [0; 108];
    fault.copy_from_slice(&many.0[260..368]);
    let mut carrier = [0; 108];
    carrier.copy_from_slice(&many.0[368..476]);
    many.0[256..4096].fill(0);
    put(&mut many.0, 48, 4 + 34 * 108);
    put(&mut many.0, 256, 34);
    for index in 0..34 {
        let start = 260 + index * 108;
        let module = if index == 0 { &fault } else { &carrier };
        many.0[start..start + 108].copy_from_slice(module);
        if index != 0 {
            let base = 0x3000_u64 + index as u64 * 0x1000;
            many.0[start..start + 8].copy_from_slice(&base.to_le_bytes());
        }
        put(&mut many.0, start + 80, if index == 0 { 4000 } else { 4032 });
    }
    for (offset, guid) in [(4000, 3), (4032, 8)] {
        many.0[offset..offset + 4].copy_from_slice(b"RSDS");
        many.0[offset + 4..offset + 20].fill(guid);
        put(&mut many.0, offset + 20, 1);
    }
    let frame = extract(&mut many, 4096, Some(33), failure())
        .map_err(|cause| std::io::Error::other(format!("actual index33 frame: {cause:?}")))?;
    assert_eq!(frame.image_base(), 0x1000);
    assert_eq!(frame.pdb(), PdbIdentity::new([3; 16], 1)?);
    Ok(())
}
#[test]
fn fault_module_range_is_start_inclusive_and_end_exclusive() {
    for (address, accepted) in [(0x1000_u64, true), (0x103f, true), (0x1040, false)] {
        let mut bytes = split_images(1);
        bytes.0[88..96].copy_from_slice(&address.to_le_bytes());
        let frame = extract(&mut bytes, 768, Some(1), FailureEvidence::Unexplained);
        assert_eq!(frame.is_ok(), accepted);
        if !accepted {
            assert_eq!(frame, Err(SymbolFrameError::UnknownModule));
        }
    }
}
#[test]
fn overlapping_fault_ranges_are_rejected_even_with_valid_annotation_and_codeview() {
    for annotation in [0, 1] {
        let mut bytes = split_images(annotation);
        let start = 260 + annotation as usize * 108;
        bytes.0[start..start + 8].copy_from_slice(&0x1000_u64.to_le_bytes());
        assert_eq!(
            extract(&mut bytes, 768, Some(annotation), failure()),
            Err(SymbolFrameError::UnknownModule)
        );
    }
}
#[test]
fn annotation_bounds_malformed_ranges_and_fault_codeview_fail_closed() {
    let mut bytes = split_images(1);
    assert_eq!(extract(&mut bytes, 768, Some(2), failure()), Err(SymbolFrameError::Corrupt));
    for (base, size) in [(u64::MAX - 8, 64_u32), (0x3000, 0)] {
        let mut bytes = split_images(1);
        bytes.0[368..376].copy_from_slice(&base.to_le_bytes());
        put(&mut bytes.0, 376, size);
        assert_eq!(
            extract(&mut bytes, 768, Some(1), failure()),
            Err(SymbolFrameError::UnknownModule)
        );
    }
    let mut bytes = split_images(1);
    put(&mut bytes.0, 48, 219);
    assert_eq!(extract(&mut bytes, 768, Some(1), failure()), Err(SymbolFrameError::Corrupt));
    let mut bytes = split_images(1);
    put(&mut bytes.0, 340, u32::MAX);
    assert_eq!(extract(&mut bytes, 768, Some(1), failure()), Err(SymbolFrameError::Corrupt));
    let mut bytes = split_images(1);
    put(&mut bytes.0, 620, 0);
    assert_eq!(extract(&mut bytes, 768, Some(1), failure()), Err(SymbolFrameError::CodeView));
}
#[test]
fn later_module_read_failure_cannot_be_hidden_by_first_matching_range() {
    struct FailedSecond(Bytes);
    impl Read for FailedSecond {
        fn bytes(&mut self, offset: u64, output: &mut [u8]) -> Result<(), SymbolFrameError> {
            if offset == 368 {
                return Err(SymbolFrameError::Source(DumpReadCode::Control));
            }
            self.0.bytes(offset, output)
        }
    }
    let mut source = FailedSecond(split_images(1));
    assert_eq!(
        extract(&mut source, 768, Some(1), failure()),
        Err(SymbolFrameError::Source(DumpReadCode::Control))
    );
}
