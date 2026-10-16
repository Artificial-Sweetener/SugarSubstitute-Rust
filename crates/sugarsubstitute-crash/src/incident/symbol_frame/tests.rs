// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only byte reads while running the real bounded exception-frame extractor.
use super::*;
struct Bytes(Vec<u8>);
impl Read for Bytes {
    fn bytes(&mut self, offset: u64, output: &mut [u8]) -> Result<(), SymbolFrameError> {
        let start = usize::try_from(offset).map_err(|_| SymbolFrameError::Corrupt)?;
        let end = start.checked_add(output.len()).ok_or(SymbolFrameError::Corrupt)?;
        let source = self.0.get(start..end).ok_or(SymbolFrameError::Corrupt)?;
        output.copy_from_slice(source);
        Ok(())
    }
}
fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn input() -> Bytes {
    let mut b = vec![0; 512];
    b[..4].copy_from_slice(b"MDMP");
    put(&mut b, 4, 0xa793);
    put(&mut b, 8, 2);
    put(&mut b, 12, 32);
    put(&mut b, 32, 6);
    put(&mut b, 36, 168);
    put(&mut b, 40, 64);
    put(&mut b, 44, 4);
    put(&mut b, 48, 112);
    put(&mut b, 52, 256);
    put(&mut b, 72, 0xc0000005);
    b[88..96].copy_from_slice(&0x1008u64.to_le_bytes());
    put(&mut b, 256, 1);
    b[260..268].copy_from_slice(&0x1000u64.to_le_bytes());
    put(&mut b, 268, 64);
    put(&mut b, 336, 24);
    put(&mut b, 340, 400);
    b[400..404].copy_from_slice(b"RSDS");
    b[404..420].copy_from_slice(&[3; 16]);
    put(&mut b, 420, 1);
    Bytes(b)
}
fn failure() -> FailureEvidence {
    FailureEvidence::NativeFault { code: 0xc0000005, address: 0x1008 }
}
#[test]
fn captured_exception_and_complete_codeview_are_from_same_selected_module() {
    let mut source = input();
    let result = extract(&mut source, 512, Some(0), failure());
    assert!(result.is_ok());
    if let Ok(frame) = result {
        assert_eq!((frame.address(), frame.image_base(), frame.image_size()), (0x1008, 0x1000, 64));
        assert_eq!(frame.pdb().guid(), &[3; 16]);
    }
}
#[test]
fn wrong_authoritative_exception_and_foreign_module_are_rejected() {
    assert_eq!(
        extract(
            &mut input(),
            512,
            Some(0),
            FailureEvidence::NativeFault { code: 0xc0000006, address: 0x1008 },
        ),
        Err(SymbolFrameError::ExceptionMismatch)
    );
    assert_eq!(
        extract(
            &mut input(),
            512,
            Some(0),
            FailureEvidence::NativeFault { code: 0xc0000005, address: 0x1009 },
        ),
        Err(SymbolFrameError::ExceptionMismatch)
    );
    let mut b = input();
    b.0[260..268].copy_from_slice(&0x2000u64.to_le_bytes());
    assert_eq!(extract(&mut b, 512, Some(0), failure()), Err(SymbolFrameError::UnknownModule));
}
#[test]
fn duplicate_streams_and_codeview_rvas_preserve_fail_closed_bounds() {
    let mut b = input();
    put(&mut b.0, 44, 6);
    assert_eq!(extract(&mut b, 512, Some(0), failure()), Err(SymbolFrameError::Corrupt));
    let mut b = input();
    put(&mut b.0, 340, u32::MAX);
    assert_eq!(extract(&mut b, 512, Some(0), failure()), Err(SymbolFrameError::Corrupt));
    let mut b = input();
    put(&mut b.0, 420, 0);
    assert_eq!(extract(&mut b, 512, Some(0), failure()), Err(SymbolFrameError::CodeView));
}

#[test]
fn categorical_evidence_retains_its_mechanism_while_reading_actual_exception_facts()
-> Result<(), Box<dyn std::error::Error>> {
    for evidence in [
        FailureEvidence::Unexplained,
        FailureEvidence::RustPanic,
        FailureEvidence::Abort,
        FailureEvidence::Startup(
            sugarsubstitute_contracts::startup::StartupFailureCode::Presentation,
        ),
    ] {
        let original = evidence;
        let frame = extract(&mut input(), 512, Some(0), evidence)
            .map_err(|cause| std::io::Error::other(format!("bounded frame: {cause:?}")))?;
        assert_eq!(
            (frame.code(), frame.address(), frame.image_base()),
            (0xc0000005, 0x1008, 0x1000)
        );
        assert_eq!(frame.pdb(), PdbIdentity::new([3; 16], 1)?);
        assert_eq!(evidence, original);
    }
    Ok(())
}
#[test]
fn unexplained_evidence_grants_no_missing_stream_foreign_module_or_corrupt_codeview() {
    let mut missing = input();
    put(&mut missing.0, 32, 9);
    assert_eq!(
        extract(&mut missing, 512, Some(0), FailureEvidence::Unexplained),
        Err(SymbolFrameError::Corrupt)
    );
    let mut foreign = input();
    foreign.0[260..268].copy_from_slice(&0x2000u64.to_le_bytes());
    assert_eq!(
        extract(&mut foreign, 512, Some(0), FailureEvidence::Unexplained),
        Err(SymbolFrameError::UnknownModule)
    );
    let mut corrupt = input();
    put(&mut corrupt.0, 420, 0);
    assert_eq!(
        extract(&mut corrupt, 512, Some(0), FailureEvidence::Unexplained),
        Err(SymbolFrameError::CodeView)
    );
}
#[path = "tests/receiving.rs"]
mod receiving;

#[path = "tests/selection.rs"]
mod selection;

#[test]
fn finished_token_frame_without_exposed_carrier_index_still_requires_unique_actual_fault_image()
-> Result<(), Box<dyn std::error::Error>> {
    let frame = extract(&mut input(), 512, None, FailureEvidence::Unexplained)
        .map_err(|cause| std::io::Error::other(format!("finished frame: {cause:?}")))?;
    assert_eq!((frame.code(), frame.address()), (0xc0000005, 0x1008));
    let mut missing = input();
    put(&mut missing.0, 44, 9);
    assert_eq!(
        extract(&mut missing, 512, None, FailureEvidence::Unexplained),
        Err(SymbolFrameError::Corrupt)
    );
    let mut foreign = input();
    foreign.0[260..268].copy_from_slice(&0x2000u64.to_le_bytes());
    assert_eq!(
        extract(&mut foreign, 512, None, FailureEvidence::Unexplained),
        Err(SymbolFrameError::UnknownModule)
    );
    assert_eq!(
        extract(&mut input(), 512, Some(1), FailureEvidence::Unexplained),
        Err(SymbolFrameError::Corrupt)
    );
    Ok(())
}

#[test]
fn finished_frame_retains_actual_header_timestamp_independently_of_fault_status() {
    let mut bytes = input();
    put(&mut bytes.0, 20, 1_791_234_567);
    let frame = extract(&mut bytes, 512, None, FailureEvidence::Unexplained).ok();
    assert_eq!(frame.map(|frame| frame.captured_at().unix_seconds()), Some(1_791_234_567));
}
