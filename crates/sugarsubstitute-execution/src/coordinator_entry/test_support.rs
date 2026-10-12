// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only external pipe effects while real execution admission and native dispatch own custody.
use crate::coordinator_entry::*;
use crate::runtime_test_support::{ClockPort, Gate, Rig, TestResult, spec};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    configuration::*, coordinator_entry::*, execution::*, lifecycle::*,
};
pub(super) fn facts() -> TestResult<EntryNativeIdentityFacts> {
    Ok(EntryNativeIdentityFacts::new(
        NamespaceId::new([1; 32]),
        &[1, 0, 0, 0, 0, 0, 0, 5],
        1,
        [4; 24],
    )?)
}
pub(super) fn owner() -> TestResult<CoordinatorKey> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
pub(super) fn run() -> TestResult<RunKey> {
    Ok(RunKey {
        owner: owner()?,
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(9)?,
    })
}
pub(super) fn request() -> TestResult<EntryLaunchRequest> {
    #[cfg(windows)]
    let path = PathBuf::from(r"E:\entry-fixture");
    #[cfg(not(windows))]
    let path = PathBuf::from("/entry-fixture");
    let config = StartupConfiguration::new(
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://localhost:8188", 8188)?),
        ProfileId::new("profile_1".into())?,
        Presentation::new("en", Theme::Light, false)?,
    );
    Ok(EntryLaunchRequest::new(
        config,
        path.clone(),
        path.clone(),
        path.clone(),
        path.clone(),
        path,
        vec![65, 61, 0xd800, 0, 0],
    )?)
}
pub(super) fn source() -> TestResult<CounterDeadline> {
    Ok(CounterDeadline::new(30_000_001_000, 1_000_000_000)?)
}
fn failure(stage: &'static str) -> EntryTransportError {
    EntryTransportError::native(NativeObservationError::new(
        stage,
        std::io::Error::other("controlled external boundary failure"),
    ))
}
#[path = "test_support/boundary.rs"]
mod boundary;
pub(super) use boundary::{Boundary, Lease};
pub(super) struct ReleaseBoundary(pub(super) Arc<Boundary>);
impl Drop for ReleaseBoundary {
    fn drop(&mut self) {
        self.0.allow_retire.store(true, Ordering::Release);
        self.0.issue_release.open();
        self.0.wake();
    }
}
pub(super) fn native_scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?)
}
pub(super) fn server(
    rig: &Rig,
    boundary: &Arc<Boundary>,
) -> TestResult<(Scope, CoordinatorEntryServerSession)> {
    let mut spec = spec(1, JobClass::NativeObservation, MIN_ENTRY_SERVER_BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    let scope = native_scope(rig)?;
    let session = reserve_server(
        &rig.client.core,
        &scope,
        spec,
        CoordinatorEntryServerInput::new(facts()?)?,
        Moment::from_nanos(30_000_000_000),
        Box::new(Lease(boundary.clone())),
    )?;
    Ok((scope, session))
}
pub(super) fn fact(
    rig: &Rig,
    control: &CoordinatorEntryControl,
    select: impl Fn(&EntryTransportObservation) -> bool,
) -> TestResult<EntryTransportObservation> {
    loop {
        let observed = rig.wake.count();
        while let Some(fact) = control.take_observation() {
            if select(&fact) {
                return Ok(fact);
            }
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        while let Some(fact) = control.take_observation() {
            if select(&fact) {
                return Ok(fact);
            }
        }
        rig.wake.wait_after(observed).map_err(|cause| {let state=lock(&control.shared.state);std::io::Error::other(format!("{cause}; prepared={}, stopped={}, owner={}, queue={}, slot0 id={:?}, peer={}, reading={}, writing={}, loan={}, header={}, until={:?}",state.prepared,state.stopped,state.owner.is_some(),state.observations.len(),state.connections.first().and_then(|slot|slot.id),state.connections.first().is_some_and(|slot|slot.peer.is_some()),state.connections.first().is_some_and(|slot|slot.reading),state.connections.first().is_some_and(|slot|slot.writing),state.connections.first().is_some_and(|slot|slot.loan),state.connections.first().is_some_and(|slot|slot.header.is_some()),state.connections.first().and_then(|slot|slot.until)))})?;
    }
}
pub(super) fn activate(rig: &Rig, session: &CoordinatorEntryServerSession) -> TestResult {
    fact(rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Prepared))?;
    session.control.activate_server(owner()?, Moment::from_nanos(30_000_000_000))?;
    fact(rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ServerActivated { .. })
    })?;
    Ok(())
}
pub(super) fn rejected_connection(
    rig: &Rig,
    control: &CoordinatorEntryControl,
    original: Option<EntryConnectionId>,
) -> TestResult<Vec<ExecutionError>> {
    let mut failures = Vec::new();
    loop {
        let observed = rig.wake.count();
        while let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            failures.push(error);
        }
        while let Some(fact) = control.take_observation() {
            assert!(!matches!(fact, EntryTransportObservation::RequestAuthenticated { .. }));
            if matches!(fact,EntryTransportObservation::ConnectionRetired {connection} if Some(connection)!=original)
            {
                while let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
                    failures.push(error);
                }
                return Ok(failures);
            }
        }
        rig.wake.wait_after(observed)?;
    }
}
