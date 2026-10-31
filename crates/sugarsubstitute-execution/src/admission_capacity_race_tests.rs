// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Control native callback return boundaries to exercise real terminal-context and shutdown races.

use super::*;
use crate::runtime::ExecutionOwner;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct ControlledWake {
    calls: AtomicUsize,
    entered: [Gate; 2],
    release: [Gate; 2],
}
impl CompletionWake for ControlledWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let index = self.calls.fetch_add(1, Ordering::AcqRel);
        if index < 2 {
            // Deliberately hold an external callback boundary to prove the owner
            // neither holds Core state across it nor claims premature settlement.
            self.entered[index].open();
            self.release[index].wait().map_err(|error| {
                ExecutionError::new(
                    Operation::NativeLifecycle,
                    Box::new(std::io::Error::other(error.to_string())),
                )
            })?;
        }
        Ok(())
    }
}

#[test]
fn terminal_context_release_and_entered_capacity_wake_remain_owned_through_shutdown() -> TestResult
{
    let wake = Arc::new(ControlledWake::default());
    let owner =
        ExecutionOwner::new(Arc::new(ClockPort(Arc::new(ManualClock::default()))), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        spec(1, JobClass::InfrastructureLifecycle, LIMIT)?,
        |_| Ok::<_, ()>(()),
    )?;
    wake.entered[0].wait()?;
    drop(job.try_take_completion().ok_or("Terminal result not published before native wake")?);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, LIMIT);
    assert!(matches!(
        client.submit_async(&scope, spec(2, JobClass::IoLifecycle, 16)?, |_| async {
            Ok::<_, ()>(())
        }),
        Err(AdmissionError::MemorySaturated { .. })
    ));
    assert!(client.drain_completions(NonZeroUsize::MIN)?.notifications.is_empty());
    wake.release[0].open();
    wake.entered[1].wait()?;
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    let mut owner = owner;
    owner.request_shutdown(Moment::from_nanos(0));
    let progress = owner.poll_shutdown()?;
    assert!(!progress.settled);
    assert!(progress.unresolved_job_ids.is_empty());
    assert_eq!((progress.queued, progress.running), (0, 0));
    assert!(client.drain_completions(NonZeroUsize::MIN)?.notifications.is_empty());
    wake.release[1].open();
    drop(owner);
    // Terminal and capacity wakes, then the single worker-return wake.
    assert_eq!(wake.calls.load(Ordering::Acquire), 3);
    assert!(client.core_for_test().settled());
    Ok(())
}

#[test]
fn shutdown_suppresses_a_capacity_wake_already_selected_for_outside_lock_delivery() -> TestResult {
    let mut rig = Rig::new()?;
    let core = rig.client.core_for_test();
    // Exercise the exact private commit/dispatch split with the same live owner.
    // This deterministically pauses at the gap that a concurrent shutdown may win.
    let selected = core.select_capacity_wake(&mut lock(&core.state));
    assert!(selected);
    assert!(!core.settled());
    rig.owner.request_shutdown(Moment::from_nanos(0));
    rig.wake.fail.store(true, Ordering::Release);
    core.deliver_capacity_wake(selected);
    assert_eq!(rig.wake.count(), 0);
    // The suppressed capacity wake stays undelivered; worker return then wakes exactly once.
    rig.wake.fail.store(false, Ordering::Release);
    drop(rig.owner);
    assert_eq!(rig.wake.count(), 1);
    assert!(rig.client.drain_completions(NonZeroUsize::MIN)?.notifications.is_empty());
    assert!(core.settled());
    Ok(())
}
