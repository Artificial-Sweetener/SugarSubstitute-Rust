// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the serving dispatcher while only its external backend settlement is controlled.
use super::progress_retired_record;
use std::{cell::Cell, error::Error};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Native retirement keeps checking the same pending backend on later serving turns.
#[test]
fn retired_serving_record_rechecks_pending_settlement_after_boundary_release() -> Result {
    let released = Cell::new(false);
    let calls = Cell::new(0);
    let mut closed = false;
    for expected in 1..=3 {
        let live = progress_retired_record(&mut closed, true, || {
            calls.set(calls.get() + 1);
            Ok(released.get())
        })?;
        assert!(!live);
        assert!(!closed);
        assert_eq!(calls.get(), expected);
    }
    released.set(true);
    assert!(!progress_retired_record(&mut closed, true, || {
        calls.set(calls.get() + 1);
        Ok(released.get())
    })?);
    assert!(closed);
    assert_eq!(calls.get(), 4);
    assert!(!progress_retired_record(&mut closed, true, || {
        calls.set(calls.get() + 1);
        Ok(false)
    })?);
    assert!(closed);
    assert_eq!(calls.get(), 4);
    Ok(())
}

/// A rejected settlement returns the original owned cause and permits a later recheck.
#[test]
fn retired_serving_record_retains_original_error_without_claiming_settlement() -> Result {
    let original: Box<dyn Error> = Box::new(std::io::Error::other("original backend settlement"));
    let original_identity = std::ptr::from_ref(original.as_ref());
    let mut closed = false;
    let returned = progress_retired_record(&mut closed, true, || Err(original))
        .err()
        .ok_or("Actual backend settlement error was not returned")?;
    assert!(std::ptr::eq(std::ptr::from_ref(returned.as_ref()), original_identity));
    assert!(!closed);
    assert!(!progress_retired_record(&mut closed, true, || Ok(true))?);
    assert!(closed);
    assert!(std::ptr::eq(std::ptr::from_ref(returned.as_ref()), original_identity));
    Ok(())
}

/// Closed and live native records leave the external backend settlement untouched.
#[test]
fn retired_serving_record_dispatch_preserves_closed_and_live_owners() -> Result {
    let called = Cell::new(false);
    for retired in [false, true] {
        let mut closed = true;
        assert!(!progress_retired_record(&mut closed, retired, || {
            called.set(true);
            Ok(false)
        })?);
        assert!(closed);
        assert!(!called.get());
    }
    let mut closed = false;
    assert!(progress_retired_record(&mut closed, false, || {
        called.set(true);
        Ok(true)
    })?);
    assert!(!closed);
    assert!(!called.get());
    Ok(())
}
