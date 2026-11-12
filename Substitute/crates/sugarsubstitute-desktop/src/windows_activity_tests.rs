// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual Win32 message classification and callback custody with CPU boundary fakes.
use super::*;
use sugarsubstitute_contracts::lifecycle::UiWatchdogActivity;

fn sent(counter: &Cell<usize>) -> Result<(), DesktopError> {
    counter.set(counter.get() + 1);
    Ok(())
}

#[test]
fn duplicate_native_flood_has_one_outstanding_wake_even_after_unrelated_drain()
-> Result<(), Box<dyn std::error::Error>> {
    let mailbox = Mailbox::new();
    let sends = Cell::new(0);
    for _ in 0..10_000 {
        mailbox.change(|signals| signals.surface(0, SurfaceFacts::observed(true, false, true)));
        mailbox.wake(|| sent(&sends));
    }
    assert_eq!(sends.get(), 1);
    assert_eq!(mailbox.signals.try_borrow_mut()?.take(), Some(UiWatchdogActivity::VisibleActive));
    mailbox.change(|signals| signals.suspend(0, Suspension::Menu(true)));
    mailbox.wake(|| sent(&sends));
    assert_eq!(sends.get(), 1);
    mailbox.consumed_wake();
    assert_eq!(mailbox.signals.try_borrow_mut()?.take(), Some(UiWatchdogActivity::Suspended));
    mailbox.wake(|| sent(&sends));
    assert_eq!(sends.get(), 1);
    Ok(())
}

#[test]
fn transition_flood_preserves_fixed_history_and_first_failure_with_one_wake()
-> Result<(), Box<dyn std::error::Error>> {
    let mailbox = Mailbox::new();
    let sends = Cell::new(0);
    for index in 0..10_000 {
        mailbox.change(|signals| {
            signals.surface(0, SurfaceFacts::observed(true, false, index % 2 == 0))
        });
        mailbox.wake(|| sent(&sends));
    }
    assert_eq!(sends.get(), 1);
    let mut signals = mailbox.signals.try_borrow_mut()?;
    assert!(
        mailbox.failure.take().is_some_and(|cause| cause.to_string().contains("history exhausted"))
    );
    for index in 0..16 {
        assert_eq!(
            signals.take(),
            Some(if index % 2 == 0 {
                UiWatchdogActivity::VisibleActive
            } else {
                UiWatchdogActivity::Inactive
            })
        );
    }
    assert!(signals.take().is_none());
    Ok(())
}

#[test]
fn actual_wake_consumption_allows_one_causal_reentrant_wake()
-> Result<(), Box<dyn std::error::Error>> {
    let mailbox = Mailbox::new();
    let sends = Cell::new(0);
    mailbox.change(|signals| signals.surface(0, SurfaceFacts::observed(true, false, true)));
    mailbox.wake(|| {
        sent(&sends)?;
        mailbox.wake(|| sent(&sends));
        Ok(())
    });
    assert_eq!(sends.get(), 1);
    mailbox.consumed_wake();
    assert_eq!(mailbox.signals.try_borrow_mut()?.take(), Some(UiWatchdogActivity::VisibleActive));
    mailbox.change(|signals| signals.suspend(0, Suspension::Move(true)));
    mailbox.wake(|| sent(&sends));
    mailbox.change(|signals| signals.suspend(0, Suspension::Move(false)));
    mailbox.wake(|| sent(&sends));
    assert_eq!(sends.get(), 2);
    assert_eq!(mailbox.signals.try_borrow_mut()?.take(), Some(UiWatchdogActivity::Suspended));
    assert_eq!(mailbox.signals.try_borrow_mut()?.take(), Some(UiWatchdogActivity::VisibleActive));
    mailbox.wake(|| sent(&sends));
    assert_eq!(sends.get(), 2);
    Ok(())
}

#[test]
fn send_failure_fences_flood_and_retains_original_cause() {
    let mailbox = Mailbox::new();
    let sends = Cell::new(0);
    mailbox.retain(DesktopError::message("first", "original cause"));
    mailbox.wake(|| {
        sends.set(sends.get() + 1);
        Err(DesktopError::message("send", "closed queue"))
    });
    mailbox.consumed_wake();
    for _ in 0..10_000 {
        mailbox.change(|signals| signals.surface(0, SurfaceFacts::observed(true, false, true)));
        mailbox.wake(|| sent(&sends));
    }
    assert_eq!(sends.get(), 1);
    assert!(!mailbox.admitting.get());
    assert!(
        mailbox.failure.take().is_some_and(|cause| cause.to_string().contains("original cause"))
    );
}

#[test]
fn history_failure_precedes_subsequent_transport_failure() {
    let mailbox = Mailbox::new();
    for index in 0..18 {
        mailbox.change(|signals| {
            signals.surface(0, SurfaceFacts::observed(true, false, index % 2 == 0));
        });
    }
    mailbox.wake(|| Err(DesktopError::message("send", "closed queue")));
    assert!(
        mailbox.failure.take().is_some_and(|cause| cause.to_string().contains("history exhausted"))
    );
    assert!(!mailbox.admitting.get());
}

#[test]
fn close_fence_precedes_failed_signal_borrow_and_preserves_callback_custody()
-> Result<(), Box<dyn std::error::Error>> {
    let mailbox = Rc::new(Mailbox::new());
    mailbox.callbacks.set(1);
    let callback = CallbackGuard(mailbox.clone());
    let held = mailbox.signals.try_borrow_mut()?;
    mailbox.close();
    mailbox.consumed_wake();
    let sends = Cell::new(0);
    mailbox.wake(|| sent(&sends));
    mailbox.change(|signals| signals.surface(0, SurfaceFacts::observed(true, false, true)));
    assert_eq!(sends.get(), 0);
    assert_eq!(mailbox.callbacks.get(), 1);
    assert!(
        mailbox
            .failure
            .take()
            .is_some_and(|cause| cause.to_string().contains("native_activity_close"))
    );
    drop(held);
    drop(callback);
    assert_eq!(mailbox.callbacks.get(), 0);
    Ok(())
}
#[test]
fn only_acknowledged_native_messages_admit_suspension() {
    for (message, value) in [(WM_ENTERMENULOOP, 0), (WM_ENTERSIZEMOVE, 0), (WM_POWERBROADCAST, 4)] {
        assert!(suspension(message, value).is_some());
    }
    for (message, value) in [
        (WM_EXITMENULOOP, 0),
        (WM_EXITSIZEMOVE, 0),
        (WM_POWERBROADCAST, 6),
        (WM_POWERBROADCAST, 7),
        (WM_POWERBROADCAST, 18),
    ] {
        assert!(suspension(message, value).is_some());
    }
    for (message, value) in
        [(WM_POWERBROADCAST, 0), (WM_POWERBROADCAST, 2), (WM_POWERBROADCAST, 19), (WM_ACTIVATE, 4)]
    {
        assert!(suspension(message, value).is_none());
    }
}
#[test]
fn callback_depth_outlives_reentrant_removal_until_chain_returns() {
    let mailbox = Rc::new(Mailbox::new());
    mailbox.callbacks.set(1);
    let outer = CallbackGuard(mailbox.clone());
    mailbox.callbacks.set(2);
    let reentrant = CallbackGuard(mailbox.clone());
    drop(reentrant);
    assert_eq!(mailbox.callbacks.get(), 1);
    drop(outer);
    assert_eq!(mailbox.callbacks.get(), 0);
}
#[test]
fn unexpected_borrow_preserves_first_failure_without_callback_panic()
-> Result<(), Box<dyn std::error::Error>> {
    let mailbox = Mailbox::new();
    let held = mailbox.signals.try_borrow_mut()?;
    mailbox.change(Signals::close);
    mailbox.retain(DesktopError::message("later", "second cause"));
    assert!(
        mailbox
            .failure
            .take()
            .is_some_and(|cause| cause.to_string().contains("native_activity_borrow"))
    );
    drop(held);
    Ok(())
}
