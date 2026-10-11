// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bounded native ABI bytes without invoking Windows security APIs.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
const ACCOUNT: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 42, 0, 0, 0];
#[test]
fn expected_descriptor_has_exact_owner_protected_full_access_and_medium_label() -> TestResult {
    let bytes = expected(ACCOUNT)?;
    assert!(bytes.len() < 512);
    assert_eq!(owner(&bytes)?, ACCOUNT);
    assert_eq!(short(&bytes, 2)?, CONTROL);
    let dacl = offset(&bytes, 16)?;
    let sacl = offset(&bytes, 12)?;
    assert_eq!(short(&bytes, dacl + 4)?, 2);
    assert_eq!(short(&bytes, sacl + 4)?, 1);
    assert_eq!(&bytes[dacl + 8..dacl + 12], &[0, 3, 24, 0]);
    assert_eq!(&bytes[sacl + 8..sacl + 12], &[17, 0, 20, 0]);
    verify(&bytes, ACCOUNT)?;
    Ok(())
}
#[test]
fn changed_owner_acl_label_or_unprotected_descriptor_fails_closed() -> TestResult {
    let bytes = expected(ACCOUNT)?;
    let dacl = offset(&bytes, 16)?;
    let sacl = offset(&bytes, 12)?;
    let owner_offset = offset(&bytes, 4)?;
    for index in [owner_offset + 8, dacl + 9, dacl + 12, dacl + 16, sacl + 8, sacl + 12, sacl + 20]
    {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert!(verify(&changed, ACCOUNT).is_err());
    }
    let mut changed = bytes.clone();
    changed[3] &= !0x10;
    assert!(verify(&changed, ACCOUNT).is_err());
    Ok(())
}
#[test]
fn malformed_foreign_ranges_ace_counts_and_capacity_reject_before_copying() -> TestResult {
    let bytes = expected(ACCOUNT)?;
    let dacl = offset(&bytes, 16)?;
    for field in [4, 12, 16] {
        let mut changed = bytes.clone();
        changed[field..field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(verify(&changed, ACCOUNT).is_err());
    }
    for (field, value) in [(dacl + 2, u16::MAX), (dacl + 4, u16::MAX), (dacl + 10, u16::MAX)] {
        let mut changed = bytes.clone();
        changed[field..field + 2].copy_from_slice(&value.to_le_bytes());
        assert!(verify(&changed, ACCOUNT).is_err());
    }
    assert!(verify(&vec![0; MAXIMUM_BYTES + 1], ACCOUNT).is_err());
    for length in 0..bytes.len() {
        assert!(verify(&bytes[..length], ACCOUNT).is_err());
    }
    assert!(expected(&[1; 69]).is_err());
    Ok(())
}
