// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep delayed exit notification independent of capture cleanup without extending either owner.
use super::*;
use std::sync::atomic::Ordering;

#[test]
fn queued_exit_can_be_prepared_after_capture_grace_without_renewing_either_deadline() -> Result<()>
{
    let (mut child, owner, clock) = super::super::test_support::child()?;
    let client = owner.client();
    child.bootstrap.channel_mut().output_polling_fixture(&client, ConnectionId::new(1)?)?;
    child.shutdown.intent = Some(CompletionIntent::StartupCancelled);
    child.shutdown.queue(Control::Activity {
        revision: NonZeroU64::new(1).ok_or("Revision absent")?,
        activity: UiWatchdogActivity::VisibleActive,
    })?;
    child.queue_exit_intent(true)?;
    let original = child.shutdown.intent_deadline.ok_or("Exit notification cutoff absent")?;
    let budget = ShutdownBudget::new(client.now()?)?;
    child.shutdown.retain_budget(budget);

    clock.0.store(12_000_000_000, Ordering::SeqCst);
    child.queue_exit_intent(true)?;
    assert!(client.now()? > budget.graceful_until);
    assert!(client.now()? < budget.settle_until);
    assert!(matches!(take_first(&mut child.shutdown.queued), Some(Control::Activity { .. })));
    let control = take_first(&mut child.shutdown.queued).ok_or("Queued exit disappeared")?;
    assert!(matches!(control, Control::ExitIntent));
    let prepared = child.prepare_control(control)?;
    assert_eq!(prepared.original, original);
    assert!(child.bootstrap.channel_mut().request_prepared(prepared.header)?);
    assert!(!prepared.admitted);
    assert_eq!(child.shutdown.budget, Some(budget));
    assert!(!child.shutdown.intent_done);
    assert!(!child.shutdown.native_retired);

    clock.0.store(31_000_000_000, Ordering::SeqCst);
    child.queue_exit_intent(true)?;
    assert_eq!(child.shutdown.intent_deadline, Some(original));
    assert!(child.prepare_control(Control::ExitIntent).is_err());
    Ok(())
}

#[test]
fn notification_queue_contention_does_not_restart_its_original_interval() -> Result<()> {
    let mut state = ChildShutdown::new();
    state.intent = Some(CompletionIntent::Normal);
    for revision in 1..=5 {
        state.queue(Control::Activity {
            revision: NonZeroU64::new(revision).ok_or("Revision absent")?,
            activity: UiWatchdogActivity::VisibleActive,
        })?;
    }
    assert!(state.request_exit_intent(true, || Ok(Moment::from_nanos(0))).is_err());
    let original = state.intent_deadline;
    assert!(state.shutdown.is_none());
    let _retained = take_first(&mut state.queued);
    state.request_exit_intent(true, || Err("Queued notification resampled its clock".into()))?;
    assert_eq!(state.intent_deadline, original);
    assert!(state.shutdown.is_some());
    assert!(!state.intent_done);
    Ok(())
}
