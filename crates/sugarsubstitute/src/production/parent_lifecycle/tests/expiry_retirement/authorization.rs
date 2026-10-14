// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ordinary checked issued replies do not acquire new native Main authorization.
use super::*;
use std::cell::Cell;
use sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError;

#[test]
fn checked_ordinary_reply_skips_closed_current_admission_and_has_no_main_effect() -> Result<()> {
    let (mut parent, h, _) = expired()?;
    let (index, _) = parent.requests.admit(retirement_header(h)?, b"original release")?;
    parent.requests.get_mut(index)?.reply = Some(Reply::Released);
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(7, 104);
    receipt.issue(7)?;
    let current_called = Cell::new(false);
    let clock_called = Cell::new(false);
    let before = parent.main.snapshot().identity;
    parent
        .apply_issued_authorization(
            index,
            |_, _| {
                current_called.set(true);
                Err(LifecyclePipeError::Closed.into())
            },
            || {
                clock_called.set(true);
                Err(LifecyclePipeError::Closed.into())
            },
        )
        .map_err(|e| e as Box<dyn Error>)?;
    assert!(!current_called.get() && !clock_called.get());
    assert_eq!(parent.main.snapshot().identity, before);
    assert!(parent.environment.is_none() && parent.browser.is_none());
    assert!(!parent.closed);
    Ok(())
}

#[test]
fn environment_and_browser_original_replies_keep_closed_current_admission_strict() -> Result<()> {
    for browser in [false, true] {
        let (mut parent, h, _) = expired()?;
        let (index, _) =
            parent.requests.admit(retirement_header(h)?, b"original native authorization")?;
        parent.requests.get_mut(index)?.reply = Some(if browser {
            Reply::Browser {
                identity: identity()?,
                pid: std::num::NonZeroU32::new(1).ok_or("PID")?,
            }
        } else {
            Reply::Environment(identity()?)
        });
        let called = Cell::new(false);
        let clock_called = Cell::new(false);
        let error = parent
            .apply_issued_authorization(
                index,
                |_, _| {
                    called.set(true);
                    Err(LifecyclePipeError::Closed.into())
                },
                || {
                    clock_called.set(true);
                    Ok(Moment::from_nanos(1))
                },
            )
            .err()
            .ok_or("Closed authorization accepted")?;
        assert_eq!(error.downcast_ref::<LifecyclePipeError>(), Some(&LifecyclePipeError::Closed));
        assert!(called.get() && !clock_called.get());
        assert!(parent.environment.is_none() && parent.browser.is_none());
    }
    Ok(())
}

#[test]
fn foreign_original_header_and_native_operation_cannot_use_ordinary_reply_classification()
-> Result<()> {
    let (mut parent, h, _) = expired()?;
    let foreign = WireHeader { connection: Some(ConnectionId::new(2)?), ..retirement_header(h)? };
    let (index, _) = parent.requests.admit(foreign, b"foreign original operation")?;
    parent.requests.get_mut(index)?.reply = Some(Reply::Released);
    let called = Cell::new(false);
    assert!(
        parent
            .apply_issued_authorization(
                index,
                |_, _| {
                    called.set(true);
                    Ok(())
                },
                || Ok(Moment::from_nanos(1)),
            )
            .is_err()
    );
    assert!(!called.get());
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(7, 104);
    assert_eq!(receipt.issue(8), Err(ParentError::WrongPhase));
    assert!(receipt.complete(7, 104).is_err());
    Ok(())
}
