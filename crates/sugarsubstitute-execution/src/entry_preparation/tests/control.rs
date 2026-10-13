// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve already adopted native outcomes across fallible external controls.

use super::super::{
    control::EntryPreparationControlCause,
    state::{CaptureDisposition, EntryEnvironmentPhase},
};
use super::support::{Behavior, capture};

#[test]
fn revoked_original_precontrol_prevents_getter_effect() {
    let (mut state, calls) = capture(vec![0, 0], Behavior::default());
    assert_eq!(
        state.capture(&mut || Err(EntryPreparationControlCause::Cancelled)),
        CaptureDisposition::Failed
    );
    assert_eq!(crate::scope::lock(&calls).get, 0);
    assert_eq!(crate::scope::lock(&calls).free, 0);
}

#[test]
fn getter_error_precedes_panicked_postcontrol() {
    let (mut state, calls) = capture(vec![0, 0], Behavior { null: true, ..Behavior::default() });
    let mut samples = 0;
    let mut control = || {
        samples += 1;
        if samples == 2 {
            std::panic::resume_unwind(Box::new("synthetic postcontrol panic"));
        }
        Ok(())
    };
    assert_eq!(state.capture(&mut control), CaptureDisposition::Failed);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("get_null"));
    assert!(state.secondary[0].as_ref().and_then(|cause| cause.panic_cause()).is_some());
    assert_eq!(crate::scope::lock(&calls).free, 0);
}

#[test]
fn cancellation_after_getter_still_frees_actual_acquired_block() {
    let (mut state, calls) = capture(vec![0, 0], Behavior::default());
    let mut samples = 0;
    let mut control = || {
        samples += 1;
        if samples >= 2 { Err(EntryPreparationControlCause::Cancelled) } else { Ok(()) }
    };
    assert_eq!(state.capture(&mut control), CaptureDisposition::Failed);
    assert_eq!(state.phase(), EntryEnvironmentPhase::Released);
    assert_eq!(crate::scope::lock(&calls).free, 1);
    assert_eq!(crate::scope::lock(&calls).reads, 0);
}

#[test]
fn free_error_precedes_panicked_postcontrol_and_never_retries() {
    let behavior = Behavior { free_fails: true, ..Behavior::default() };
    let (mut state, calls) = capture(vec![0, 0], behavior);
    let calls_for_control = calls.clone();
    let mut control = || {
        if crate::scope::lock(&calls_for_control).free == 1 {
            std::panic::resume_unwind(Box::new("synthetic post-free panic"));
        }
        Ok(())
    };
    assert_eq!(state.capture(&mut control), CaptureDisposition::RetirementUncertain);
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("invalid_native_state"));
    assert!(state.secondary[0].as_ref().and_then(|cause| cause.panic_cause()).is_some());
    state.retire(&mut || Ok(()));
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn checked_free_does_not_erase_postcontrol_failure_or_reopen_copy() {
    let (mut state, calls) = capture(vec![0, 0], Behavior::default());
    let calls_for_control = calls.clone();
    let mut control = || {
        if crate::scope::lock(&calls_for_control).free == 1 {
            Err(EntryPreparationControlCause::Deadline)
        } else {
            Ok(())
        }
    };
    assert_eq!(state.capture(&mut control), CaptureDisposition::Failed);
    assert_eq!(state.phase(), EntryEnvironmentPhase::Released);
    assert!(state.take_units().is_none());
    assert_eq!(crate::scope::lock(&calls).get, 1);
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn repeated_capture_never_gets_a_second_original_block() {
    let (mut state, calls) = capture(vec![0, 0], Behavior::default());
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Copied);
    assert_eq!(state.capture(&mut || Ok(())), CaptureDisposition::Failed);
    assert_eq!(crate::scope::lock(&calls).get, 1);
    assert_eq!(crate::scope::lock(&calls).free, 1);
}

#[test]
fn secondary_inventory_stays_bounded_and_preserves_first_cause() {
    let (mut state, _) = capture(vec![0, 0], Behavior::default());
    for _ in 0..32 {
        state.record(super::super::cause::EntryEnvironmentCause::InvalidNativeState);
    }
    assert_eq!(state.first_cause().map(|cause| cause.category()), Some("invalid_native_state"));
    assert_eq!(state.secondary.iter().filter(|cause| cause.is_some()).count(), 8);
    assert_eq!(state.omitted, 23);
}
