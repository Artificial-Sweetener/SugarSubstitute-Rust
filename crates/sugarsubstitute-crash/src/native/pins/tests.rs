// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake raw values test durable adoption; these cases issue no native call.
use super::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn rejected_first_raw_output_cannot_lose_second_actual_output() -> TestResult {
    let mut pins = Pins::new()?;
    pins.adopt(41)?;
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
    let mut prefix = prefix()?;
    prefix.push(&path);
    assert_eq!(prefix.capacity(), MAX_UNITS * 3);
    assert_eq!(prefix.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    let wide = wide(&path)?;
    assert_eq!(wide.capacity(), MAX_UNITS + 1);
    assert_eq!(&wide[..wide.len() - 1], units);
    let pins = Pins::new()?;
    assert_eq!(pins.values.capacity(), MAX_HANDLES);
    let bridge_peak = 5 * MAX_UNITS * 3
        + 4 * wide.capacity() * size_of::<u16>()
        + pins.values.capacity() * size_of::<Option<usize>>()
        + 16 * 1024;
    assert_eq!(bridge_peak, 786458);
    assert!(bridge_peak < crate::input::NATIVE_STORAGE_BYTES);
    Ok(())
}

#[test]
fn actual_oversized_standard_canonical_capacity_rejects_with_transient_storage_covered()
-> TestResult {
    let units: Vec<u16> =
        "E:\\".encode_utf16().chain(std::iter::repeat_n(0xd800, MAX_UNITS - 3)).collect();
    let actual = PathBuf::from(OsString::from_wide(&units));
    assert_eq!(actual.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(actual.capacity(), 131064);
    assert!(canonical(&units).is_err());
    let rejection_peak = 6 * MAX_UNITS * 3
        + actual.capacity()
        + (MAX_UNITS + 1) * size_of::<u16>()
        + MAX_HANDLES * size_of::<Option<usize>>()
        + 16 * 1024;
    assert_eq!(rejection_peak, 819218);
    assert_eq!(crate::input::NATIVE_STORAGE_BYTES - rejection_peak, 147438);
    let mut pins = Pins::new()?;
    let slots = pins.reserve_outputs()?;
    pins.retain_outputs(slots, [41, 42])?;
    assert!(canonical(&units).is_err());
    assert_eq!(pins.values[slots[0]], Some(41));
    assert_eq!(pins.values[slots[1]], Some(42));
    pins.values.clear();
    Ok(())
}

#[test]
fn bounded_canonical_conversion_preserves_isolated_surrogates_and_rejects_nul() -> TestResult {
    let units = [b'E' as u16, b':' as u16, b'\\' as u16, 0xd800, b'a' as u16, 0xdc00];
    let actual = canonical(&units)?;
    assert!(actual.to_str().is_none());
    assert_eq!(actual.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert!(canonical(&[b'E' as u16, b':' as u16, b'\\' as u16, 0]).is_err());
    Ok(())
}
