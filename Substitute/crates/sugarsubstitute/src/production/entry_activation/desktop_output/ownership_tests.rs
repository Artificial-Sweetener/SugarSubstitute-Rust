// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify stream outcomes without minting process/native settlement from copied facts.
use super::*;
#[test]
fn natural_output_completion_is_not_a_desktop_exit_failure_or_global_shutdown() {
    assert!(terminal_issue(TerminalKind::Success, false).is_none());
    assert!(terminal_issue(TerminalKind::Success, true).is_none());
    assert!(terminal_issue(TerminalKind::Cancelled, true).is_none());
    for kind in [
        TerminalKind::Failure,
        TerminalKind::Panicked,
        TerminalKind::TimedOut,
        TerminalKind::Cancelled,
    ] {
        assert!(matches!(terminal_issue(kind,false),Some(Issue::Stopped(actual)) if actual==kind));
    }
    for kind in [TerminalKind::Failure, TerminalKind::Panicked, TerminalKind::TimedOut] {
        assert!(matches!(terminal_issue(kind,true),Some(Issue::Stopped(actual)) if actual==kind));
    }
}
#[test]
fn actual_cancelled_scope_rejects_desktop_output_before_native_effects_and_retains_no_reservation()
-> Result<()> {
    use std::sync::Arc;
    use sugarsubstitute_contracts::{execution::*, lifecycle::*};
    use sugarsubstitute_execution::ExecutionOwner;
    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> Moment {
            Moment::from_nanos(1)
        }
        fn sleep_until(&self, _: Moment) -> WaitFuture {
            Box::pin(std::future::pending())
        }
    }
    struct Wake;
    impl CompletionWake for Wake {
        fn wake(&self) -> std::result::Result<(), ExecutionError> {
            Ok(())
        }
    }
    let owner = ExecutionOwner::new(Arc::new(FixedClock), Arc::new(Wake))?;
    let client = owner.client();
    let scope =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    scope.cancel();
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let jobs = JobSequence::new();
    assert!(DesktopOutput::new(run, &client, &scope, &jobs).is_err());
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(scope.snapshot().queued, 0);
    drop(owner);
    Ok(())
}

#[test]
fn launched_output_drain_uses_only_original_settlement_cutoff() -> Result<()> {
    use sugarsubstitute_contracts::{execution::Moment, lifecycle::ShutdownBudget};
    let original = ShutdownBudget::new(Moment::from_nanos(1))?;
    assert!(drain_deadline_issue(original.started, original, false).is_none());
    assert!(drain_deadline_issue(original.graceful_until, original, false).is_none());
    assert!(matches!(
        drain_deadline_issue(original.settle_until, original, false),
        Some(Issue::Stopped(TerminalKind::TimedOut))
    ));
    Ok(())
}
#[test]
fn checked_natural_retirement_does_not_create_a_late_output_timeout() -> Result<()> {
    use sugarsubstitute_contracts::{execution::Moment, lifecycle::ShutdownBudget};
    let original = ShutdownBudget::new(Moment::from_nanos(1))?;
    assert!(drain_deadline_issue(original.settle_until, original, true).is_none());
    assert!(terminal_issue(TerminalKind::Cancelled, false).is_some());
    assert!(terminal_issue(TerminalKind::Failure, true).is_some());
    Ok(())
}
