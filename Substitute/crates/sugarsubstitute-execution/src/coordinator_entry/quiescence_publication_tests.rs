// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the real final publication wake before the worker can continue its retirement transition.
use super::kernel::*;
use crate::{
    ExecutionOwner,
    coordinator_entry::*,
    runtime_test_support::{ClockPort, Gate, ManualClock, Rig, TestResult, Wake},
    scope::lock,
};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU8, Ordering},
};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};

struct PublicationWake {
    delegate: Arc<Wake>,
    control: Mutex<Option<Weak<CoordinatorEntryControl>>>,
    observed: AtomicU8,
    published: Gate,
}
impl CompletionWake for PublicationWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        if let Some(control) = lock(&self.control).as_ref().and_then(Weak::upgrade) {
            let state = lock(&control.shared.state);
            if let Some(index) = state.observations.iter().find_map(|event| {
                matches!(event.observation, EntryTransportObservation::ConnectionRetired { .. })
                    .then_some(event.slot)
                    .flatten()
            }) {
                let slot = &state.connections[index];
                let cleared = slot.id.is_none()
                    && slot.peer.is_none()
                    && slot.header.is_none()
                    && slot.until.is_none()
                    && slot.read.is_none()
                    && slot.reply.is_none()
                    && !slot.reading
                    && !slot.writing
                    && slot.loan;
                self.observed.store(if cleared { 1 } else { 2 }, Ordering::Release);
                self.published.open();
            }
        }
        self.delegate.wake()
    }
}
#[test]
fn normal_idle_final_retirement_publication_exposes_cleared_identity_and_original_retired_loan()
-> TestResult {
    let clock = Arc::new(ManualClock::default());
    let wake = Arc::new(Wake::default());
    let observer = Arc::new(PublicationWake {
        delegate: wake.clone(),
        control: Mutex::new(None),
        observed: AtomicU8::new(0),
        published: Gate::default(),
    });
    let executor =
        ExecutionOwner::with_cpu_workers(Arc::new(ClockPort(clock.clone())), observer.clone(), 2)?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner: executor, client, clock, wake, scope };
    let boundary = Boundary::new(&rig, true)?;
    boundary.hold_peer_close.store(true, Ordering::Release);
    let (_scope, session) = server(&rig, &boundary)?;
    let CoordinatorEntryServerSession { control, mut completion } = session;
    let control = Arc::new(control);
    *lock(&observer.control) = Some(Arc::downgrade(&control));
    fact(&rig, &control, |fact| matches!(fact, EntryTransportObservation::Prepared))?;
    control.activate_server(owner()?, Moment::from_nanos(30_000_000_000))?;
    fact(&rig, &control, |fact| matches!(fact, EntryTransportObservation::ServerActivated { .. }))?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { connection, request } =
        fact(&rig, &control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Original authenticated source missing".into());
    };
    let original = request.until();
    assert!(!control.try_close_when_idle()?);
    control.reply(
        connection,
        request.request_id(),
        EntryOperationId::new(1)?,
        Moment::from_nanos(1_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(&rig, &control, |fact| matches!(fact, EntryTransportObservation::Written { .. }))?;
    boundary.peer_retirement_issued.wait()?;
    let client = rig.client.clone();
    // The external native retirement result arms the existing mailbox for exactly the final publication.
    // This changes no production clock, queue, state, native result or frame custody.
    *lock(&boundary.retirement_probe) = Some(Box::new(move || {
        let drained = client.drain_completions(std::num::NonZeroUsize::MIN);
        assert!(drained.is_ok());
    }));
    boundary.close_peer(0)?;
    observer.published.wait()?;
    let observed = observer.observed.load(Ordering::Acquire);
    fact(
        &rig,
        &control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==connection),
    )?;
    control.close();
    boundary.retiring.wait()?;
    assert!(completion.try_take_completion().is_none());
    assert_eq!(request.until(), original);
    assert_eq!(request.view()?.profile(), "profile_1");
    drop(request);
    assert!(
        matches!(rig.completion(&mut completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==1 && report.retired_connections==1)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    assert_eq!(
        observed, 1,
        "Final publication wake exposed old live identities before their checked retirement clear"
    );
    Ok(())
}
