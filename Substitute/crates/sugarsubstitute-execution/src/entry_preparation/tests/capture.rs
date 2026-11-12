// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual copying, policy rejection and independent checked retirement.

use super::super::state::{CaptureDisposition, EntryEnvironmentPhase};
use super::support::{Behavior, capture};
use sugarsubstitute_contracts::coordinator_entry::MAX_ENTRY_ENVIRONMENT_UNITS;

#[test]
fn original_order_hidden_drive_and_unpaired_surrogate_are_preserved() {
    let units = vec![61, 67, 58, 61, 90, 0, 90, 61, 0xd800, 0, 65, 61, 50, 0, 0];
    let (mut state, calls) = capture(units.clone(), Behavior::default());
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Copied);
    assert_eq!(state.phase(), EntryEnvironmentPhase::Released);
    assert_eq!(state.take_units(), Some(units));
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn native_empty_block_preserves_actual_two_zeros() {
    let (mut state, calls) = capture(vec![0, 0], Behavior::default());
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Copied);
    assert_eq!(state.take_units(), Some(vec![0, 0]));
    assert_eq!(crate::scope::lock(&calls).reads, 4);
}

#[test]
fn copied_policy_limit_includes_original_terminal_zeros() {
    let mut units = vec![65; MAX_ENTRY_ENVIRONMENT_UNITS];
    units[MAX_ENTRY_ENVIRONMENT_UNITS - 2] = 0;
    units[MAX_ENTRY_ENVIRONMENT_UNITS - 1] = 0;
    let (mut state, calls) = capture(units.clone(), Behavior::default());
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Copied);
    assert_eq!(state.take_units(), Some(units));
    assert_eq!(crate::scope::lock(&calls).reads, 2 * MAX_ENTRY_ENVIRONMENT_UNITS);
}

#[test]
fn double_zero_beyond_policy_cap_is_rejected_without_extra_read() {
    let mut units = vec![65; MAX_ENTRY_ENVIRONMENT_UNITS + 1];
    units[MAX_ENTRY_ENVIRONMENT_UNITS - 1] = 0;
    units[MAX_ENTRY_ENVIRONMENT_UNITS] = 0;
    let (mut state, calls) = capture(units, Behavior::default());
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Failed);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("too_large"));
    assert!(state.units.is_none());
    assert_eq!(crate::scope::lock(&calls).reads, MAX_ENTRY_ENVIRONMENT_UNITS);
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn first_original_double_zero_stops_before_any_suffix() {
    let (mut state, calls) = capture(vec![65, 61, 66, 0, 0, 0xd800], Behavior::default());
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Copied);
    assert_eq!(state.take_units(), Some(vec![65, 61, 66, 0, 0]));
    assert_eq!(crate::scope::lock(&calls).reads, 10);
}

#[test]
fn null_getter_is_failure_and_never_fabricates_empty_copy() {
    let (mut state, calls) = capture(vec![0, 0], Behavior { null: true, ..Behavior::default() });
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Failed);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("get_null"));
    assert!(state.take_units().is_none());
    assert_eq!(crate::scope::lock(&calls).free, 0);
}

#[test]
fn acquired_pointer_survives_getter_unwind_until_actual_cleanup() {
    let behavior = Behavior { getter_panics_after_acquisition: true, ..Behavior::default() };
    let (mut state, calls) = capture(vec![0, 0], behavior);
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Failed);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("provider_panic"));
    assert_eq!(state.phase(), EntryEnvironmentPhase::Released);
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn original_read_error_is_retained_before_cleanup() {
    let behavior = Behavior { read_fails_at: Some(1), ..Behavior::default() };
    let (mut state, calls) = capture(vec![0, 0], behavior);
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Failed);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("invalid_native_state"));
    assert_eq!(crate::scope::lock(&calls).free, 1);
    assert!(state.take_units().is_none());
}

#[test]
fn failed_free_keeps_copy_and_never_retries_or_delivers_it() {
    let behavior = Behavior { free_fails: true, ..Behavior::default() };
    let (mut state, calls) = capture(vec![0, 0], behavior);
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::RetirementUncertain);
    assert_eq!(state.units.as_deref(), Some([0, 0].as_slice()));
    assert!(state.take_units().is_none());
    state.retire(&mut || Ok(()));
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn panicked_free_keeps_actual_uncertain_custody() {
    let behavior = Behavior { free_panics: true, ..Behavior::default() };
    let (mut state, calls) = capture(vec![0, 0], behavior);
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::RetirementUncertain);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("provider_panic"));
    state.retire(&mut || Ok(()));
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn getter_panic_remains_primary_when_actual_cleanup_also_fails() {
    let behavior =
        Behavior { getter_panics_after_acquisition: true, free_fails: true, ..Behavior::default() };
    let (mut state, calls) = capture(vec![0, 0], behavior);
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::RetirementUncertain);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("provider_panic"));
    assert_eq!(
        state.secondary[0].as_ref().map(|cause| cause.category()),
        Some("invalid_native_state")
    );
    assert!(state.units.is_none());
    state.retire(&mut || Ok(()));
    assert_eq!(crate::scope::lock(&calls).free, 1);
}
