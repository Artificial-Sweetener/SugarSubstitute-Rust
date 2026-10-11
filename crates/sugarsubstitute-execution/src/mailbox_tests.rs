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
