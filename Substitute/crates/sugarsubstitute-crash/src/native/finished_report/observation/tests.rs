// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize data-only native cause ordering and malformed result preservation.
use super::*;
fn result() -> Output {
    let mut result = Output::empty();
    result.version = 1;
    result.status = 7;
    result
}
#[test]
fn native_primary_and_control_secondary_keep_reported_provenance() -> Result<(), InvalidResult> {
    let mut result = result();
    result.primary_error = 31;
    result.primary_origin = 1;
    result.cleanup_count = 1;
    result.cleanup_codes[0] = 995;
    result.cleanup_origin_bits = 2;
    let mut failure = Failures::new();
    failure.observe(&result, Some(ControlCause::Cancelled))?;
    assert!(matches!(failure.first, Some(Failure::Native { origin: Origin::Native, code: 31 })));
    assert!(matches!(failure.secondary[0], Some(Failure::Control(ControlCause::Cancelled))));
    Ok(())
}
#[test]
fn control_first_remains_original_typed_cause() -> Result<(), InvalidResult> {
    let mut result = result();
    result.primary_error = 1460;
    result.primary_origin = 2;
    let mut failure = Failures::new();
    failure.observe(&result, Some(ControlCause::TimedOut))?;
    assert!(matches!(failure.first, Some(Failure::Control(ControlCause::TimedOut))));
    Ok(())
}
#[test]
fn cumulative_result_repeat_preserves_original_control_without_requiring_new_source()
-> Result<(), InvalidResult> {
    let mut result = result();
    result.primary_error = 995;
    result.primary_origin = 2;
    let mut failure = Failures::new();
    failure.observe(&result, Some(ControlCause::Cancelled))?;
    failure.observe(&result, None)?;
    assert!(matches!(failure.first, Some(Failure::Control(ControlCause::Cancelled))));
    assert!(failure.secondary.iter().all(Option::is_none));
    Ok(())
}
#[test]
fn malformed_origin_retains_actual_control_and_never_mutates_native_token() {
    let mut result = result();
    result.lease = 73;
    result.primary_error = 31;
    result.primary_origin = 4;
    let mut failure = Failures::new();
    assert!(failure.observe(&result, Some(ControlCause::Cancelled)).is_err());
    assert!(matches!(failure.first, Some(Failure::Invalid(_))));
    assert!(matches!(failure.secondary[0], Some(Failure::Control(ControlCause::Cancelled))));
    assert_eq!(result.lease, 73);
}
#[test]
fn claimed_control_code_must_match_the_original_typed_source() {
    let mut result = result();
    result.primary_error = 995;
    result.primary_origin = 2;
    let mut failure = Failures::new();
    assert!(failure.observe(&result, Some(ControlCause::TimedOut)).is_err());
    assert!(matches!(failure.first, Some(Failure::Control(ControlCause::TimedOut))));
    assert!(matches!(failure.secondary[0], Some(Failure::Invalid(_))));
}
