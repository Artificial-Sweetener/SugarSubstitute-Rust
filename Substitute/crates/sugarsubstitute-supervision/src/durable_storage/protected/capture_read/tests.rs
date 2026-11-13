// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original source credit and retirement fences without native directory facts.
use super::*;
#[path = "native_tests.rs"]
mod native;
#[test]
fn actual_root_reader_pin_blocks_close_and_preserves_original_credit() -> TestResult {
    admitted(|context| {
        let mut root = ProtectedStorageRoot::new(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_claim", cause))?,
        )?;
        let pin = root
            .state
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("root absent"))?
            .readers
            .acquire()?;
        assert!(!root.is_retired());
        assert!(root.retire(&context).is_err());
        assert!(context.memory().claim(1).is_err());
        assert!(
            root.state
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("root absent"))?
                .readers
                .acquire()
                .is_err()
        );
        drop(pin);
        root.retire(&context)?;
        assert_eq!(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_claim", cause))?
                .bytes(),
            STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn actual_root_last_source_fence_keeps_original_claim_after_checked_root_retirement() -> TestResult
{
    admitted(|context| {
        let mut root = ProtectedStorageRoot::new(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_claim", cause))?,
        )?;
        let pin = root
            .state
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("root absent"))?
            .readers
            .acquire()?;
        let fence = pin.fence();
        drop(pin);
        root.retire(&context)?;
        assert!(context.memory().claim(1).is_err());
        assert!(!fence.open());
        drop(fence);
        assert_eq!(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_claim", cause))?
                .bytes(),
            STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn actual_root_reader_bound_rejects_fifth_pin_without_changing_custody() -> TestResult {
    admitted(|context| {
        let mut root = ProtectedStorageRoot::new(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_claim", cause))?,
        )?;
        let readers =
            &root.state.as_ref().ok_or_else(|| ClaimError::invalid("root absent"))?.readers;
        let pins = [readers.acquire()?, readers.acquire()?, readers.acquire()?, readers.acquire()?];
        assert_eq!(readers.count(), 4);
        assert!(readers.acquire().is_err());
        assert_eq!(readers.count(), 4);
        drop(pins);
        root.retire(&context)?;
        assert_eq!(
            context
                .memory()
                .claim(STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("test_claim", cause))?
                .bytes(),
            STORAGE_BYTES
        );
        Ok(())
    })
}
