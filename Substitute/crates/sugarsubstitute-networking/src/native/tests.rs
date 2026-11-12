// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Checked native retirement preserves failed slots and parent custody.
use super::*;
use std::cell::RefCell;
#[test]
fn failed_retirement_retains_original_slots_and_never_closes_parent_first() -> Result<(), Failure> {
    let mut handles =
        Transport { request: 3, connection: 2, session: 1, idle: None, context: None };
    let calls = RefCell::new(Vec::new());
    assert!(
        handles
            .close_using(|handle| {
                calls.borrow_mut().push(handle);
                if handle == 2 { Err(Failure::invalid("injected_close_failure")) } else { Ok(()) }
            })
            .is_err()
    );
    assert_eq!(*calls.borrow(), [3, 2]);
    assert_eq!((handles.request, handles.connection, handles.session), (0, 2, 1));
    handles.close_using(|handle| {
        calls.borrow_mut().push(handle);
        Ok(())
    })?;
    assert_eq!(*calls.borrow(), [3, 2, 2, 1]);
    assert!(handles.empty());
    Ok(())
}

mod qualification;
