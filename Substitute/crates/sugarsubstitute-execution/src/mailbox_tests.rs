// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Characterize coalesced wakes and original native wake errors.

use super::*;
use crate::runtime_test_support::*;
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::*;

#[test]
fn mailbox_wakes_one_pending_batch_and_rewakes_remaining_bounded_drain() -> TestResult {
    let wake = Arc::new(Wake::default());
    let mailbox = Mailbox::new(wake.clone());
    for id in 1..=48 {
        if mailbox.enqueue(CompletionNotification {
            scope_id: ScopeId::new(1)?,
            job_id: JobId::new(id)?,
            kind: TerminalKind::Success,
            replacement: None,
        }) {
            mailbox.deliver_wake();
        }
    }
    assert_eq!(wake.count(), 1);
    let first = mailbox.drain(
        NonZeroUsize::new(100).ok_or_else(|| std::io::Error::other("Invalid test maximum"))?,
    )?;
    assert_eq!(first.notifications.len(), 32);
    assert_eq!(first.remaining, 16);
    assert_eq!(wake.count(), 2);
    let second = mailbox.drain(
        NonZeroUsize::new(32).ok_or_else(|| std::io::Error::other("Invalid test maximum"))?,
    )?;
    assert_eq!(second.notifications.len(), 16);
    assert_eq!(second.remaining, 0);
    assert_eq!(wake.count(), 2);
    assert!(mailbox.enqueue(CompletionNotification {
        scope_id: ScopeId::new(1)?,
        job_id: JobId::new(49)?,
        kind: TerminalKind::Cancelled,
        replacement: None
    }));
    Ok(())
}

#[test]
fn failed_native_wake_is_observed_without_losing_typed_terminal_result() -> TestResult {
    let rig = Rig::new()?;
    rig.wake.fail.store(true, Ordering::Release);
    let stamp = rig.wake.count();
    let mut job =
        rig.client.submit_async(&rig.scope, spec(1, JobClass::IoOrdinary, 0)?, |_| async {
            Ok::<_, ()>(17)
        })?;
    rig.wake.wait_after(stamp)?;
    let completion = job
        .try_take_completion()
        .ok_or_else(|| std::io::Error::other("Typed terminal result was lost"))?;
    assert!(matches!(completion.outcome, JobOutcome::Success(17)));
    drop(rig.owner);
    let error = rig
        .client
        .drain_completions(NonZeroUsize::MIN)
        .err()
        .ok_or_else(|| std::io::Error::other("Wake failure was not retained"))?;
    assert_eq!(error.operation(), Operation::NativeLifecycle);
    assert!(error.to_string().contains("Owned wake fixture failed"));
    rig.wake.fail.store(false, Ordering::Release);
    Ok(())
}

#[test]
fn consumed_fatal_wake_allows_a_later_terminal_notification() -> TestResult {
    let wake = Arc::new(Wake::default());
    let mailbox = Mailbox::new(wake.clone());
    assert!(mailbox.signal_capacity());
    mailbox.deliver_wake();
    mailbox.record_failure(crate::runtime::error(std::io::Error::other("original owner failure")));
    let failure = mailbox.drain(NonZeroUsize::MIN).err().ok_or("Fatal cause lost")?;
    assert!(failure.to_string().contains("original owner failure"));
    assert_eq!(wake.count(), 1);
    assert!(mailbox.enqueue(CompletionNotification {
        scope_id: ScopeId::new(1)?,
        job_id: JobId::new(1)?,
        kind: TerminalKind::Panicked,
        replacement: None,
    }));
    mailbox.deliver_wake();
    assert_eq!(wake.count(), 2);
    let terminal = mailbox.drain(NonZeroUsize::MIN)?;
    assert_eq!(terminal.notifications.len(), 1);
    assert_eq!(terminal.notifications[0].kind, TerminalKind::Panicked);
    assert_eq!(terminal.remaining, 0);
    assert!(mailbox.drain(NonZeroUsize::MIN)?.notifications.is_empty());
    assert_eq!(wake.count(), 2);
    Ok(())
}

#[derive(Default)]
struct InspectingWake {
    mailbox: Mutex<Option<std::sync::Weak<Mailbox>>>,
    calls: std::sync::atomic::AtomicU64,
    locked: std::sync::atomic::AtomicBool,
}
impl CompletionWake for InspectingWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if let Some(mailbox) = lock(&self.mailbox).as_ref().and_then(std::sync::Weak::upgrade)
            && mailbox.pending.try_lock().is_err()
        {
            self.locked.store(true, Ordering::Release);
        }
        Ok(())
    }
}

#[test]
fn fatal_drain_rearms_retained_terminal_backlog_outside_the_mailbox_lock() -> TestResult {
    let wake = Arc::new(InspectingWake::default());
    let mailbox = Arc::new(Mailbox::new(wake.clone()));
    *lock(&wake.mailbox) = Some(Arc::downgrade(&mailbox));
    for id in 1..=40 {
        if mailbox.enqueue(CompletionNotification {
            scope_id: ScopeId::new(1)?,
            job_id: JobId::new(id)?,
            kind: TerminalKind::Failure,
            replacement: None,
        }) {
            mailbox.deliver_wake();
        }
    }
    assert_eq!(wake.calls.load(Ordering::Acquire), 1);
    mailbox.record_failure(crate::runtime::error(std::io::Error::other("original queued failure")));
    let failure = mailbox.drain(NonZeroUsize::MIN).err().ok_or("Fatal cause lost")?;
    assert!(failure.to_string().contains("original queued failure"));
    assert_eq!(wake.calls.load(Ordering::Acquire), 2);
    let maximum = NonZeroUsize::MIN.saturating_add(99);
    let first = mailbox.drain(maximum)?;
    assert_eq!(first.notifications.len(), 32);
    assert_eq!(first.remaining, 8);
    assert_eq!(first.notifications[0].job_id, JobId::new(1)?);
    assert_eq!(first.notifications[31].job_id, JobId::new(32)?);
    assert_eq!(wake.calls.load(Ordering::Acquire), 3);
    let last = mailbox.drain(maximum)?;
    assert_eq!(last.notifications.len(), 8);
    assert_eq!(last.notifications[0].job_id, JobId::new(33)?);
    assert_eq!(last.notifications[7].job_id, JobId::new(40)?);
    assert_eq!(last.remaining, 0);
    assert!(!wake.locked.load(Ordering::Acquire));
    assert_eq!(wake.calls.load(Ordering::Acquire), 3);
    Ok(())
}

#[test]
fn mailbox_native_projection_borrows_original_first_leaf_through_nested_owned_aggregates()
-> TestResult {
    use crate::native::NativeObservationError;
    let first = Box::new(NativeObservationError::new(
        "execution_qpc_counter",
        std::io::Error::from_raw_os_error(233),
    ));
    let address = first.as_ref() as *const NativeObservationError;
    let mailbox = Mailbox::new(Arc::new(Wake::default()));
    mailbox.record_failure(crate::runtime::aggregate_errors(vec![ExecutionError::new(
        Operation::NativeLifecycle,
        first,
    )]));
    mailbox.record_failure(ExecutionError::new(
        Operation::NativeLifecycle,
        Box::new(NativeObservationError::new(
            "later_failure",
            std::io::Error::from_raw_os_error(5),
        )),
    ));
    let error = mailbox.drain(NonZeroUsize::MIN).err().ok_or("Mailbox failure absent")?;
    let native = NativeObservationError::from_execution_cause(&error)
        .ok_or("Original native projection absent")?;
    assert!(std::ptr::eq(address, native));
    assert_eq!(native.stage(), "execution_qpc_counter");
    assert_eq!(native.win32_code(), Some(233));
    assert!(
        NativeObservationError::from_execution_cause(native)
            .is_some_and(|actual| std::ptr::eq(actual, native))
    );
    Ok(())
}

struct OpaqueProjectionCause {
    callbacks: Arc<std::sync::atomic::AtomicUsize>,
    dropped: Arc<std::sync::atomic::AtomicUsize>,
}
impl std::fmt::Debug for OpaqueProjectionCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.callbacks.fetch_add(1, Ordering::SeqCst);
        formatter.write_str("PRIVATE")
    }
}
impl std::fmt::Display for OpaqueProjectionCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.callbacks.fetch_add(1, Ordering::SeqCst);
        formatter.write_str("PRIVATE")
    }
}
impl std::error::Error for OpaqueProjectionCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.callbacks.fetch_add(1, Ordering::SeqCst);
        Some(self)
    }
}
impl Drop for OpaqueProjectionCause {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn mailbox_native_projection_stops_at_original_opaque_cause_without_callbacks_or_custody_loss()
-> TestResult {
    use crate::native::NativeObservationError;
    let callbacks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mailbox = Mailbox::new(Arc::new(Wake::default()));
    mailbox.record_failure(ExecutionError::new(
        Operation::NativeLifecycle,
        Box::new(OpaqueProjectionCause { callbacks: callbacks.clone(), dropped: dropped.clone() }),
    ));
    mailbox.record_failure(ExecutionError::new(
        Operation::NativeLifecycle,
        Box::new(NativeObservationError::new(
            "later_native_failure",
            std::io::Error::from_raw_os_error(233),
        )),
    ));
    let error = mailbox.drain(NonZeroUsize::MIN).err().ok_or("Mailbox failure absent")?;
    assert!(NativeObservationError::from_execution_cause(&error).is_none());
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    drop(error);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    Ok(())
}
