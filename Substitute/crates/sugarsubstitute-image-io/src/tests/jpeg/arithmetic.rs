// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject dependency arithmetic outside the reviewed header subset before decoding entropy.
use super::*;

/// Construct the repeated category-11 DC predictor attack against ordinary i32 dequantization.
fn predictor_fixture() -> Vec<u8> {
    let mut bytes = vec![255, 216, 255, 219, 0, 67, 0];
    bytes.extend([255; 64]);
    bytes.extend([255, 192, 0, 11, 8, 2, 8, 2, 8, 1, 1, 17, 0]);
    for (selector, symbol) in [(0, 11), (16, 0)] {
        bytes.extend([255, 196, 0, 20, selector, 1]);
        bytes.extend([0; 15]);
        bytes.push(symbol);
    }
    bytes.extend([255, 218, 0, 8, 1, 1, 0, 0, 63, 0]);
    let mut value = 0_u8;
    let mut count = 0;
    for bit in (0..4225).flat_map(|_| std::iter::once(0).chain([1; 11]).chain([0])) {
        value = (value << 1) | bit;
        count += 1;
        if count == 8 {
            bytes.push(value);
            if value == 255 {
                bytes.push(0);
            }
            value = 0;
            count = 0;
        }
    }
    if count != 0 {
        value = (value << (8 - count)) | ((1 << (8 - count)) - 1);
        bytes.push(value);
        if value == 255 {
            bytes.push(0);
        }
    }
    bytes.extend([255, 217]);
    bytes
}

#[test]
fn jpeg_predictor_overflow_fixture_and_first_unsafe_header_reject_before_decode()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let bytes = predictor_fixture();
        assert!(
            matches!(JpegImport::open(Cursor::new(&bytes), [0; 32], context), Err(ImportError::Invalid(message)) if message.contains("arithmetic subset"))
        );
        let sof = marker(&bytes, 0xc0)?;
        let safe_blocks = (i32::MAX as u64 / (2047 * 255)) as u16;
        let mut safe = bytes.clone();
        safe[sof + 5..sof + 7].copy_from_slice(&(safe_blocks * 8).to_be_bytes());
        safe[sof + 7..sof + 9].copy_from_slice(&8_u16.to_be_bytes());
        // This characterizes only the header's arithmetic domain, not full entropy validity.
        assert!(JpegImport::open(Cursor::new(&safe), [0; 32], context).is_ok());
        safe[sof + 5..sof + 7].copy_from_slice(&((safe_blocks + 1) * 8).to_be_bytes());
        assert!(
            matches!(JpegImport::open(Cursor::new(safe), [0; 32], context), Err(ImportError::Invalid(message)) if message.contains("arithmetic subset"))
        );
        drop(context.memory().claim(2 * 1024 * 1024)?);
        Ok(())
    })
}

#[test]
fn actual_default_comfy_512_quality_95_preview_fits_the_supported_arithmetic_domain()
-> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let bytes = include_bytes!("fixtures/comfy-512.jpg");
        let import = JpegImport::open(Cursor::new(bytes), Sha256::digest(bytes).into(), context)?;
        assert_eq!(import.descriptor().layout().extent(), Extent::new(512, 512)?);
        let mut output = Vec::new();
        import.write(&mut output, context)?;
        assert!(!output.is_empty());
        Ok(())
    })
}
