// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Force late input publication between the worker's initial take and retirement.
use super::*;
use crate::runtime_test_support::{ClockPort, ManualClock, Wake};
use std::sync::Weak;

/// Interleave the real control transfer at the injected clock boundary, without
/// adding a worker hook or replacing the retirement behavior under test.
struct TransferClock {
    base: Arc<ManualClock>,
    core: Mutex<Weak<Core>>,
    stage: AtomicUsize,
    dispatch_paused: Gate,
    release_dispatch: Gate,
    transfer: Mutex<Option<LifecyclePipeControl>>,
}
impl TransferClock {
    /// Create an inert provider before any native worker exists.
    fn new(base: Arc<ManualClock>) -> Self {
        Self {
            base,
            core: Mutex::new(Weak::new()),
            stage: AtomicUsize::new(0),
            dispatch_paused: Gate::default(),
            release_dispatch: Gate::default(),
            transfer: Mutex::new(None),
        }
    }
}
impl Clock for TransferClock {
    fn now(&self) -> Moment {
        let native = lock(&self.core).upgrade().is_some_and(|core| {
            core.native_thread
                .get()
                .is_some_and(|thread| thread.id() == std::thread::current().id())
        });
        if native && self.stage.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire).is_ok()
        {
            self.dispatch_paused.open();
            if let Err(cause) = self.release_dispatch.wait() {
                std::panic::resume_unwind(Box::new(cause.to_string()));
            }
        } else if native
            && self.stage.compare_exchange(3, 4, Ordering::AcqRel, Ordering::Acquire).is_ok()
            && let Some(control) = lock(&self.transfer).take()
        {
            control.close();
            control.bind_input(PeerInput::Simulated);
        }
        ClockPort(self.base.clone()).now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        ClockPort(self.base.clone()).sleep_until(deadline)
    }
}

/// Cancellation and publication after the first input take must still adopt
/// that input before any successful native retirement or terminal delivery.
#[test]
fn input_published_at_cancellation_checkpoint_is_adopted_before_retirement() -> TestResult {
    let base = Arc::new(ManualClock::default());
    let clock = Arc::new(TransferClock::new(base.clone()));
    let wake = Arc::new(Wake::default());
    let owner = crate::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    *lock(&clock.core) = Arc::downgrade(&client.core);
    let ordinary = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client, clock: base, wake, scope: ordinary };
    let scope = scope(&rig)?;
    clock.stage.store(1, Ordering::Release);
    rig.client.core.changed();
    clock.dispatch_paused.wait()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    *lock(&clock.transfer) = Some(LifecyclePipeControl {
        shared: session.control.shared.clone(),
        job: session.control.job.clone(),
    });
    clock.stage.store(3, Ordering::Release);
    clock.release_dispatch.open();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(clock.stage.load(Ordering::Acquire), 4);
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    assert_eq!(boundary.adopted.load(Ordering::Acquire), 1);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
