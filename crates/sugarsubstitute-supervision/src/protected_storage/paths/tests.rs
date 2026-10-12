// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise raw actual-result path decoding without native filesystem effects.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn actual_native_path_decoding_preserves_isolated_high_and_low_surrogates() -> TestResult {
    let units = [92, 92, 63, 92, 69, 58, 92, 0xd800, 97, 0xdc01];
    let path = decode_native_result(&units)?;
    assert!(path.to_str().is_none());
    assert_eq!(path.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    validate_canonical_local(&path)?;
    Ok(())
}
#[test]
fn actual_native_path_decoding_keeps_exact_case_and_accepts_drive_root() -> TestResult {
    for value in [r"\\?\E:\", r"\\?\e:\MixedCase\漢字"] {
        let units = value.encode_utf16().collect::<Vec<_>>();
        let path = decode_native_result(&units)?;
        assert_eq!(path.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    }
    Ok(())
}
#[test]
fn actual_native_path_decoding_rejects_unc_device_relative_and_nul() {
    for value in [
        r"E:\regular",
        r"\\?\UNC\server\share",
        r"\\.\pipe\endpoint",
        r"\\?\E:relative",
        "\\\\?\\E:\\nul\0tail",
    ] {
        assert!(decode_native_result(&value.encode_utf16().collect::<Vec<_>>()).is_err());
    }
    assert!(decode_native_result(&[]).is_err());
}
#[test]
fn actual_native_path_decoding_enforces_exact_existing_unit_bound() -> TestResult {
    let mut units = vec![92, 92, 63, 92, 69, 58, 92];
    units.resize(32766, 97);
    assert_eq!(decode_native_result(&units)?.as_os_str().encode_wide().count(), 32766);
    units.push(97);
    assert!(decode_native_result(&units).is_err());
    Ok(())
}

#[test]
fn bounded_shell_scan_preserves_raw_local_path_units_and_rejects_missing_terminator() -> TestResult
{
    let mut units = vec![69, 58, 92, 0xd800, 97, 0xdc01, 0];
    // SAFETY: This owned buffer includes the terminator inside its exact advertised bound.
    let path = unsafe { read_native_path(PWSTR(units.as_mut_ptr()), units.len()) }?;
    assert!(path.to_str().is_none());
    assert_eq!(path.as_os_str().encode_wide().collect::<Vec<_>>(), units[..units.len() - 1]);
    // SAFETY: The scan cannot read beyond the valid owned allocation and rejects this shorter bound.
    assert!(unsafe { read_native_path(PWSTR(units.as_mut_ptr()), units.len() - 1) }.is_err());
    // SAFETY: The null pointer rejects before any dereference.
    assert!(unsafe { read_native_path(PWSTR::null(), units.len()) }.is_err());
    Ok(())
}
