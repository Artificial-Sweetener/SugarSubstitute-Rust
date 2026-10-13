// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the real transaction queue and immutable cleanup budget without native authority fakes.
use super::*;
#[test]
fn earlier_native_budget_keeps_complete_start_grace_and_total() -> Result<()> {
    let first = ShutdownBudget::new(Moment::from_nanos(10))?;
    let later = ShutdownBudget::new(Moment::from_nanos(20))?;
    let mut state = ChildShutdown::new();
    state.retain_budget(later);
    state.retain_budget(first);
    state.retain_budget(later);
    assert_eq!(state.budget, Some(first));
    assert!(!state.native_retired);
    assert!(!state.intent_done);
    assert!(!state.completed_done);
    Ok(())
}
#[test]
fn four_control_slots_reject_overload_without_erasing_original_fifo() -> Result<()> {
    let mut state = ChildShutdown::new();
    for value in 1..=4 {
        state.queue(Control::Activity {
            revision: NonZeroU64::new(value).ok_or("Revision missing")?,
            activity: UiWatchdogActivity::VisibleActive,
        })?;
    }
    assert!(state.queue(Control::ExitCompleted).is_err());
    for value in 1..=4 {
        assert!(
            matches!(take_first(&mut state.queued),Some(Control::Activity{revision,..}) if revision.get()==value)
        );
    }
    assert!(take_first(&mut state.queued).is_none());
    assert!(!state.native_retired);
    Ok(())
}
#[test]
fn queued_exit_intent_and_completion_never_prove_acknowledgement_or_retirement() -> Result<()> {
    let mut state = ChildShutdown::new();
    state.queue(Control::ExitIntent)?;
    state.queue(Control::ExitCompleted)?;
    assert!(matches!(take_first(&mut state.queued), Some(Control::ExitIntent)));
    assert!(matches!(take_first(&mut state.queued), Some(Control::ExitCompleted)));
    assert!(!state.intent_done);
    assert!(!state.completed_done);
    assert!(!state.native_retired);
    assert!(state.budget.is_none());
    Ok(())
}
