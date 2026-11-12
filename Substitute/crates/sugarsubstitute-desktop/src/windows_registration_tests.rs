// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise provisional reference transfer with synchronous native reentry fakes.

use super::*;
use std::error::Error;

#[test]
fn failed_install_returns_only_the_provisional_reference() {
    let payload = Rc::new(Cell::new(0));
    let owner = Registration::new(&payload);
    let before = Rc::strong_count(&owner.0);
    assert!(owner.install(|_| Err(DesktopError::message("fixture", "failed"))).is_err());
    assert!(!owner.installed());
    assert_eq!(Rc::strong_count(&owner.0), before);
}

#[test]
fn reentrant_final_destruction_during_failed_install_returns_reference_once() {
    let payload = Rc::new(Cell::new(0));
    let owner = Registration::new(&payload);
    assert!(
        owner
            .install(|raw| {
                // SAFETY: The fake invokes final destruction with this exact retained registration.
                unsafe {
                    reclaim::<Cell<u32>>(raw, true);
                }
                Err(DesktopError::message("fixture", "failed after destruction"))
            })
            .is_err()
    );
    assert!(owner.destroyed());
    assert!(!owner.installed());
    assert_eq!(Rc::strong_count(&owner.0), 1);
    assert!(owner.install(|_| Ok(())).is_err());
}

#[test]
fn failed_removal_retains_native_reference_until_final_destruction() -> Result<(), Box<dyn Error>> {
    let payload = Rc::new(Cell::new(0));
    let owner = Registration::new(&payload);
    owner.install(|_| Ok(()))?;
    assert!(owner.remove(|_| Err(DesktopError::message("fixture", "remove failed"))).is_err());
    assert!(owner.installed());
    assert_eq!(Rc::strong_count(&owner.0), 2);
    // SAFETY: The fake drives the same exact final reference return as WM_NCDESTROY.
    unsafe {
        reclaim::<Cell<u32>>(owner.data(), true);
    }
    let called = Cell::new(false);
    owner.remove(|_| {
        called.set(true);
        Ok(())
    })?;
    assert!(!called.get());
    assert_eq!(Rc::strong_count(&owner.0), 1);
    Ok(())
}

#[test]
fn reentrant_final_destruction_during_successful_removal_returns_once() -> Result<(), Box<dyn Error>>
{
    let payload = Rc::new(Cell::new(0));
    let owner = Registration::new(&payload);
    owner.install(|raw| {
        // SAFETY: The fake's installed concrete payload matches this registration.
        assert_eq!(unsafe { callback::<Cell<u32>>(raw) }.get(), 0);
        Ok(())
    })?;
    owner.remove(|raw| {
        // SAFETY: Reentrant destruction returns this owner's live exact native reference.
        unsafe {
            reclaim::<Cell<u32>>(raw, true);
        }
        Ok(())
    })?;
    assert_eq!(Rc::strong_count(&owner.0), 1);
    assert!(owner.destroyed());
    Ok(())
}
