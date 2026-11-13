// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share actual admitted transport fixtures with the registered recovery owner tests.
use super::*;
use crate::{RegisteredRecoveryGrant, backend_custody::*, recovery_supervisor::*};
/// Preserve the original fixture failure while identifying its actual causal setup boundary.
fn observed_stage<T>(stage: &'static str, result: TestResult<T>) -> TestResult<T> {
    result.map_err(|cause| Box::new(WaitFailure { stage, cause }) as Box<dyn std::error::Error>)
}
#[derive(Debug)]
struct WaitFailure {
    stage: &'static str,
    cause: Box<dyn std::error::Error>,
}
impl std::fmt::Display for WaitFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recovery fixture {}: {}", self.stage, self.cause)
    }
}
impl std::error::Error for WaitFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
/// Authenticate a real same-Core custody parent while replacing only native pipe effects.
pub(crate) fn recovery_parent(
    rig: &Rig,
    scope_id: u64,
    job_id: u64,
) -> TestResult<(Scope, BackendCustodyPipeSession, BackendCustodyParentStorage)> {
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None },
        None,
        true,
    )?;
    let boundary = Boundary::new();
    let key = sugarsubstitute_contracts::backend_custody::BackendCustodyKey::new(
        run()?.owner,
        sugarsubstitute_contracts::backend_custody::BackendCustodySession::new(9)?,
    );
    *lock(&boundary.binding) = Some(PeerBinding::BackendCustody(key));
    let mut spec = spec(job_id, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    let (inner, completion) = admit(
        &rig.client.core,
        &scope,
        spec,
        Some(PeerBinding::BackendCustody(key)),
        Moment::from_nanos(30_000_000_000),
        Box::new(Lease(boundary)),
        Owner::BackendCustody,
    )?;
    let mut session =
        BackendCustodyPipeSession { control: BackendCustodyPipeControl { inner }, completion };
    session.control.inner.bind_input(PeerInput::Simulated);
    loop {
        let revision = rig.wake.count();
        let mut connected = false;
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            connected |= matches!(fact, BackendCustodyPipeObservation::Connected);
        }
        if connected {
            break;
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        // The actual fact can arrive under the previous coalesced wake before its acknowledgement.
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            connected |= matches!(fact, BackendCustodyPipeObservation::Connected);
        }
        if connected {
            break;
        }
        observed_stage("custody Connected", rig.wake.wait_after(revision))?;
    }
    session.control.verify_capability([7; 32], Moment::from_nanos(30_000_000_000))?;
    loop {
        let revision = rig.wake.count();
        let mut authenticated = false;
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            authenticated |= matches!(fact, BackendCustodyPipeObservation::Authenticated(_));
        }
        if authenticated {
            break;
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        // The actual fact can arrive under the previous coalesced wake before its acknowledgement.
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            authenticated |= matches!(fact, BackendCustodyPipeObservation::Authenticated(_));
        }
        if authenticated {
            break;
        }
        observed_stage("custody Authenticated", rig.wake.wait_after(revision))?;
    }
    let parent = session.control.take_parent_storage()?;
    Ok((scope, session, parent))
}
/// Prepare the real genuine-grant server before its future supervisor exists.
pub(crate) fn recovery_server(
    rig: &Rig,
    scope_id: u64,
    job_id: u64,
    grant: &RegisteredRecoveryGrant,
) -> TestResult<(Scope, RecoveryPipeSession)> {
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None },
        None,
        true,
    )?;
    let boundary = Boundary::new();
    *lock(&boundary.binding) = Some(PeerBinding::Recovery(grant.key()));
    let mut spec = spec(job_id, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    let (inner, completion) = admit_owned(
        &rig.client.core,
        &scope,
        spec,
        PipeAdmission {
            binding: Some(PeerBinding::Recovery(grant.key())),
            handshake: grant.original_budget().until(),
            owner: Owner::Recovery,
        },
        RequestKind::RecoveryPipe(grant),
        (),
        |(), context| {
            let claim =
                context.memory().claim(65536).map_err(|e| (AdmissionError::Memory(e), ()))?;
            lock(&grant.state().map_err(|cause| (cause, ()))?.stages).pipe = Some(spec.id);
            Ok(PreparedPipeLease { lease: Box::new(Lease(boundary)), metadata: Some(claim) })
        },
    )
    .map_err(|(error, ())| error)?;
    let mut session = RecoveryPipeSession { control: RecoveryPipeControl { inner }, completion };
    loop {
        let revision = rig.wake.count();
        let mut prepared = false;
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            prepared |= matches!(fact, RecoveryPipeObservation::Prepared);
        }
        if prepared {
            break;
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        // The actual fact can arrive under the previous coalesced wake before its acknowledgement.
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            prepared |= matches!(fact, RecoveryPipeObservation::Prepared);
        }
        if prepared {
            break;
        }
        observed_stage("recovery Prepared", rig.wake.wait_after(revision))?;
    }
    Ok((scope, session))
}

/// Genuine child-side session; only immutable bootstrap/kernel effects are replaced.
pub(crate) struct RecoveryChildBoundary(Arc<Boundary>);
/// Admit and authenticate the actual child transport owner before startup assembly.
pub(crate) fn recovery_child(
    rig: &Rig,
    scope_id: u64,
    job_id: u64,
    key: sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantKey,
) -> TestResult<(Scope, RecoveryPipeSession, RecoveryChildBoundary)> {
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None },
        None,
        true,
    )?;
    let boundary = Boundary::new();
    *lock(&boundary.binding) = Some(PeerBinding::Recovery(key));
    *lock(&boundary.original_counter) =
        Some(LifecyclePipeDeadline::new(30_000_000_000, 1_000_000_000)?);
    let mut spec = spec(job_id, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    let (inner, completion) = admit(
        &rig.client.core,
        &scope,
        spec,
        None,
        Moment::from_nanos(30_000_000_000),
        Box::new(Lease(boundary.clone())),
        Owner::Recovery,
    )?;
    let mut session = RecoveryPipeSession { control: RecoveryPipeControl { inner }, completion };
    session.control.inner.bind_input(PeerInput::Simulated);
    child_fact(rig, &mut session, |fact| matches!(fact, RecoveryPipeObservation::Connected))?;
    session.control.verify_capability([7; 32], Moment::from_nanos(30_000_000_000))?;
    child_fact(rig, &mut session, |fact| {
        matches!(fact, RecoveryPipeObservation::Authenticated(_))
    })?;
    Ok((scope, session, RecoveryChildBoundary(boundary)))
}
/// Wait on the real execution wake; drain a complete fixed turn before parking.
fn child_fact(
    rig: &Rig,
    session: &mut RecoveryPipeSession,
    select: impl Fn(&RecoveryPipeObservation) -> bool,
) -> TestResult<RecoveryPipeObservation> {
    loop {
        let revision = rig.wake.count();
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            if select(&fact) {
                return Ok(fact);
            }
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        for fact in session.drain(NonZeroUsize::new(32).ok_or("Observation bound invalid")?) {
            if select(&fact) {
                return Ok(fact);
            }
        }
        rig.wake.wait_after(revision)?;
    }
}
/// Deliver through the actual native read/guard lifecycle, never a reconstructed public buffer.
pub(crate) fn recovery_frame(
    rig: &Rig,
    session: &mut RecoveryPipeSession,
    boundary: &RecoveryChildBoundary,
    operation: u64,
    key: sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantKey,
    sequence: u64,
    command: sugarsubstitute_contracts::recovery_supervisor::wire::RecoveryCommand<'_>,
) -> TestResult<ReceivedLifecycleFrame> {
    use sugarsubstitute_contracts::recovery_supervisor::wire::*;
    let mut bytes = vec![0; RECOVERY_WIRE_BYTES];
    let length = encode(
        &RecoveryFrame { key, sequence, command },
        RecoveryReceiver::Supervisor,
        &mut bytes,
    )?;
    bytes.truncate(length);
    *lock(&boundary.0.read_payload) = Some(bytes);
    session
        .control
        .read(LifecyclePipeOperationId::new(operation)?, session.control.original_deadline())?;
    match child_fact(rig, session, |fact| matches!(fact, RecoveryPipeObservation::Received { .. }))?
    {
        RecoveryPipeObservation::Received { frame, .. } => Ok(frame),
        _ => Err("Actual read missing".into()),
    }
}

#[path = "recovery_fixture/retirement_tests.rs"]
mod retirement_tests;

/// Create the ordinary direct UI server with only its native kernel boundary replaced.
pub(crate) fn prepared_direct_server(
    rig: &Rig,
    grant: &RegisteredRecoveryGrant,
    scope_id: u64,
    job_id: u64,
) -> TestResult<(
    Scope,
    LifecyclePipeSession,
    LifecyclePipeBootstrap,
    crate::lifecycle_pipe::enrollment::Enrollment,
)> {
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(scope_id)?, deadline: None },
        None,
        true,
    )?;
    let boundary = Boundary::new();
    boundary.inherited.store(20, Ordering::Release);
    *lock(&boundary.binding) = Some(PeerBinding::Desktop(grant.key().run()));
    let mut specification =
        spec(job_id, JobClass::NativeObservation, crate::lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let session = crate::lifecycle_pipe::reserve(
        &rig.client.core,
        &scope,
        specification,
        Some(grant.key().run()),
        grant.original_budget().until(),
        Box::new(Lease(boundary)),
    )?;
    let pin = loop {
        let revision = rig.wake.count();
        if let Some(pin) = session.control.take_bootstrap()? {
            break pin;
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(pin) = session.control.take_bootstrap()? {
            break pin;
        }
        rig.wake.wait_after(revision)?;
    };
    let enrollment = crate::lifecycle_pipe::enrollment::Enrollment::take(&session.control.inner)?;
    Ok((scope, session, pin, enrollment))
}
