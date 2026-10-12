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
