// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep checked connection retirement dormant while its original frame loan remains held.
use super::kernel::*;
use crate::{
    admission::Core,
    coordinator_entry::*,
    native::NativeWork,
    runtime_test_support::{ClockPort, Rig, TestResult, spec},
    task::Work,
};
use std::{
    future::Future,
    sync::Arc,
    sync::atomic::Ordering,
    task::{Context, Waker},
};
use sugarsubstitute_contracts::execution::*;

fn changed(core: &Core) -> bool {
    let mut notified = Box::pin(core.notify.notified());
    notified.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_ready()
}

#[test]
fn checked_old_connection_with_held_frame_does_not_self_wake_on_repeated_worker_progress()
-> TestResult {
    let rig = Rig::new()?;
    // Drive the sole native worker's actual Active body on a real admitted
    // Core; no concurrent supervisor consumes its observable Notify token.
    let core =
        Core::new(ExecutionId::new(99)?, Arc::new(ClockPort(rig.clock.clone())), rig.wake.clone());
    let scope =
        core.create_scope_kind(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None, true)?;
    let boundary = Boundary::new(&rig, true)?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_SERVER_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut session = reserve_server(
        &core,
        &scope,
        specification,
        CoordinatorEntryServerInput::new(facts()?)?,
        Moment::from_nanos(30_000_000_000),
        Box::new(Lease(boundary.clone())),
    )?;
    let (queued, _) =
        core.take_native_at(Moment::from_nanos(0)).ok_or("Actual native dequeue missing")?;
    let Work::Native(NativeWork::CoordinatorEntry(work)) = queued.work else {
        return Err("Wrong actual native owner selected".into());
    };
    let mut active = worker::Active::new(work, queued.context, &core, Moment::from_nanos(0));
    active.progress(&core, Moment::from_nanos(0), true);
    session.control.activate_server(owner()?, Moment::from_nanos(30_000_000_000))?;
    boundary.connect()?;
    active.progress(&core, Moment::from_nanos(0), true);
    let mut original = None;
    while let Some(fact) = session.control.take_observation() {
        if let EntryTransportObservation::RequestAuthenticated { connection, request } = fact {
            original = Some((connection, request));
        }
    }
    let (connection, request) = original.ok_or("Actual authenticated frame missing")?;
    boundary.allow_retire.store(false, Ordering::Release);
    session.control.close_connection(connection);
    assert!(changed(&core));
    assert!(!changed(&core));
    active.progress(&core, Moment::from_nanos(0), true);
    assert!(changed(&core));
    assert!(!changed(&core));
    assert_eq!(boundary.native_closes.load(Ordering::Acquire), 0);
    assert!(session.control.take_observation().is_none());
    // A genuine external completion signal still wakes pending retirement.
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake();
    assert!(changed(&core));
    active.progress(&core, Moment::from_nanos(0), true);
    let retired = session.control.take_observation().ok_or("Checked retirement not observed")?;
    assert!(
        matches!(retired, EntryTransportObservation::ConnectionRetired { connection: actual, failure: None } if actual == connection)
    );
    while changed(&core) {}
    let closes = boundary.native_closes.load(Ordering::Acquire);
    for _ in 0..3 {
        active.progress(&core, Moment::from_nanos(0), true);
        assert!(!changed(&core), "Held retired connection self-woke the native scheduler");
        assert!(session.control.take_observation().is_none());
        assert_eq!(boundary.native_closes.load(Ordering::Acquire), closes);
        assert!(session.completion.try_take_completion().is_none());
    }
    assert_eq!(request.view()?.profile(), "profile_1");
    assert_eq!(scope.snapshot().reserved_bytes, MIN_ENTRY_SERVER_BYTES);
    drop(request);
    assert!(changed(&core));
    active.progress(&core, Moment::from_nanos(0), true);
    session.control.close();
    active.progress(&core, Moment::from_nanos(0), true);
    assert!(active.completed());
    assert!(
        matches!(session.completion.try_take_completion().ok_or("Actual terminal missing")?.outcome,
        JobOutcome::Success(report) if report.authenticated_launches == 1 && report.retired_connections == 1)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
