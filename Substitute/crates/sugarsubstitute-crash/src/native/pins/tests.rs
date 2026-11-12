// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake raw values test durable adoption; these cases issue no native call.
use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn rejected_first_raw_output_cannot_lose_second_actual_output() -> TestResult {
    let mut pins = Pins::new()?;
    let first = pins.reserve_outputs()?;
    pins.retain_outputs(first, [41, 0])?;
    pins.release_unused_outputs();
    let slots = pins.reserve_outputs()?;
    assert!(pins.retain_outputs(slots, [41, 42]).is_err());
    assert_eq!(pins.values[0], Some(41));
    assert_eq!(pins.values[slots[1]], Some(42));
    pins.values.clear();
    Ok(())
}
#[test]
fn invalid_primary_still_adopts_unique_secondary_before_validation() -> TestResult {
    let mut pins = Pins::new()?;
    let slots = pins.reserve_outputs()?;
    assert!(pins.retain_outputs(slots, [usize::MAX, 42]).is_err());
    assert_eq!(pins.values[slots[1]], Some(42));
    pins.values.clear();
    Ok(())
}
#[test]
fn output_capacity_rejection_precedes_effect_and_empty_outputs_reuse_capacity() -> TestResult {
    let mut pins = Pins::new()?;
    pins.values.resize(MAX_HANDLES - 1, None);
    assert!(pins.reserve_outputs().is_err());
    assert_eq!(pins.values.len(), MAX_HANDLES - 1);
    pins.values.clear();
    for _ in 0..MAX_HANDLES {
        let slots = pins.reserve_outputs()?;
        pins.retain_outputs(slots, [0, 0])?;
        pins.release_unused_outputs();
        assert!(pins.values.is_empty());
    }
    Ok(())
}

#[test]
fn actual_maximum_path_and_wide_allocations_fit_the_native_phase_bound() -> TestResult {
    let units: Vec<u16> =
        "E:\\".encode_utf16().chain(std::iter::repeat_n(0xd800, MAX_UNITS - 3)).collect();
    let mut path = PathBuf::from(OsString::from_wide(&units));
    path.shrink_to_fit();
    assert!(path.capacity() <= MAX_UNITS * 3);
    crate::input::validate_path(&path)?;
    let wide = wide(&path)?;
    assert_eq!(wide.capacity(), MAX_UNITS + 1);
    assert_eq!(&wide[..wide.len() - 1], units);
    let pins = Pins::new()?;
    assert_eq!(pins.values.capacity(), MAX_HANDLES);
    let bridge_peak = 3 * wide.capacity() * size_of::<u16>()
        + pins.values.capacity() * size_of::<Option<usize>>()
        + 16 * 1024;
    assert_eq!(bridge_peak, 229434);
    assert!(bridge_peak < crate::input::NATIVE_STORAGE_BYTES);
    Ok(())
}

#[test]
fn actual_oversized_input_capacity_rejects_without_losing_partial_outputs() -> TestResult {
    let units: Vec<u16> =
        "E:\\".encode_utf16().chain(std::iter::repeat_n(0xd800, MAX_UNITS - 3)).collect();
    let actual = PathBuf::from(OsString::from_wide(&units));
    assert_eq!(actual.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(actual.capacity(), 131064);
    assert!(
        crate::CaptureStartInput::new(
            {
                let mut oversized = PathBuf::with_capacity(actual.capacity());
                oversized.push(&actual);
                oversized
            },
            sugarsubstitute_supervision::InstalledReleaseOwner::new().capture_selection()
        )
        .is_err()
    );
    let rejection_peak = actual.capacity()
        + (MAX_UNITS + 1) * size_of::<u16>()
        + MAX_HANDLES * size_of::<Option<usize>>()
        + 16 * 1024;
    assert!(rejection_peak < crate::input::NATIVE_STORAGE_BYTES);
    let mut pins = Pins::new()?;
    let slots = pins.reserve_outputs()?;
    pins.retain_outputs(slots, [41, 42])?;
    assert!(
        crate::CaptureStartInput::new(
            {
                let mut oversized = PathBuf::with_capacity(actual.capacity());
                oversized.push(&actual);
                oversized
            },
            sugarsubstitute_supervision::InstalledReleaseOwner::new().capture_selection()
        )
        .is_err()
    );
    assert_eq!(pins.values[slots[0]], Some(41));
    assert_eq!(pins.values[slots[1]], Some(42));
    pins.values.clear();
    Ok(())
}

#[test]
fn bounded_root_conversion_preserves_isolated_surrogates_and_rejects_nul() -> TestResult {
    let units = [b'E' as u16, b':' as u16, b'\\' as u16, 0xd800, b'a' as u16, 0xdc00];
    let actual = PathBuf::from(OsString::from_wide(&units));
    crate::input::validate_path(&actual)?;
    assert!(actual.to_str().is_none());
    assert_eq!(actual.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert!(
        crate::input::validate_path(&PathBuf::from(OsString::from_wide(&[
            b'E' as u16,
            b':' as u16,
            b'\\' as u16,
            0
        ])))
        .is_err()
    );
    Ok(())
}
