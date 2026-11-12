// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the actual connected native control with a deterministic post-preflight clock cutoff.
use super::super::super::types::WritePurpose;
use super::*;
use crate::completion_wake::Wake;
use sha2::{Digest, Sha256};
use std::{
    num::NonZeroUsize,
    os::windows::io::{AsRawHandle, IntoRawHandle},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    Clock, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Operation, ScopeId, ScopeSpec,
    WaitFuture,
};
use sugarsubstitute_contracts::lifecycle::{
    LifecycleFailure, LifecycleFailureCode, ResourceObservation,
};
use sugarsubstitute_execution::{
    ExecutionOwner,
    windows_lifecycle_pipe::{
        InheritedLifecycleBootstrap, LifecyclePipeError, LifecyclePipeObservation,
        LifecyclePipeOperationId, LifecyclePipeSession, MIN_LIFECYCLE_PIPE_BYTES,
    },
};
use sugarsubstitute_supervision::lifecycle_protocol::ReplyOutcome;

#[derive(Default)]
struct BoundaryClock(AtomicU64);
impl Clock for BoundaryClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

fn spec(id: u64) -> Result<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::NativeObservation,
        deadline: None,
        reservation_bytes: MIN_LIFECYCLE_PIPE_BYTES,
        replacement: None,
    })
}

/// Native-only fixture: real read-only bootstrap transfer and both original terminals;
/// no process/window is created and no peer/native readiness capability is manufactured.
#[test]
fn actual_control_returns_original_never_issued_buffer_after_parent_precheck_cutoff() -> Result<()>
{
    static INSTANCE: AtomicU64 = AtomicU64::new(1);
    let mut id = identity()?;
    let mut namespace = [151; 32];
    namespace[..4].copy_from_slice(&std::process::id().to_le_bytes());
    namespace[4..12].copy_from_slice(&INSTANCE.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    id.run.owner.namespace = NamespaceId::new(namespace);
    let (parent, h, lease, mut coordinator) = super::super::acquisition_progress::pending_for(id)?;
    let mut parent = Box::new(parent);
    let transition = coordinator
        .observe(ResourceObservation::Failed {
            resource: lease.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::other("original preparation producer")),
            ),
        })
        .map_err(|rejected| format!("Failure rejected: {:?}", rejected.reason))?;
    let outcome = transition
        .replies
        .iter()
        .find(|reply| Some(reply.request) == h.request)
        .ok_or("Original failed absent")?
        .outcome;
    let index = parent.requests.find(h.request.ok_or("Origin")?).ok_or("Slot")?;
    parent.requests.observe_backend(index, outcome)?;
    let acq = parent.acquisition.as_mut().ok_or("Original")?;
    acq.observe_backend(h, outcome)?;
    acq.reply(h, Reply::Failed(LifecycleFailureCode::Preparation))?;
    parent.requests.get_mut(index)?.reply = Some(Reply::Failed(LifecycleFailureCode::Preparation));
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(BoundaryClock::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut server = client.reserve_lifecycle_pipe_server(
        &scope,
        spec(1)?,
        id.run,
        Moment::from_nanos(1_000_000_000_000),
    )?;
    let bootstrap = loop {
        let observed = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(63))?;
        if let Some(bootstrap) = server.control.take_bootstrap()? {
            break bootstrap;
        }
        if let Some(terminal) = server.completion.try_take_completion() {
            return Err(format!(
                "Original native server preparation terminated: {:?}",
                terminal.outcome
            )
            .into());
        }
        wake.wait_after(observed);
    };
    let permit = client.reserve_lifecycle_pipe_client(
        &scope,
        spec(2)?,
        bootstrap.deadline().ok_or("Deadline")?,
    )?;
    let duplicate = bootstrap.borrow_inheritance().section.try_clone_to_owned()?;
    let token = InheritedLifecycleBootstrap::new(duplicate.as_raw_handle() as u64)?;
    let _transferred = duplicate.into_raw_handle();
    let mut peer = permit.bind(token).ok_or("Permit lost")?;
    let mut connected = false;
    loop {
        let observed = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(63))?;
        while let Some(observation) = peer.control.take_observation() {
            connected |= matches!(observation, LifecyclePipeObservation::Connected);
        }
        if connected {
            break;
        }
        if let Some(terminal) = peer.completion.try_take_completion() {
            return Err(format!(
                "Original native peer connection terminated: {:?}",
                terminal.outcome
            )
            .into());
        }
        if let Some(terminal) = server.completion.try_take_completion() {
            return Err(format!(
                "Original native server connection terminated: {:?}",
                terminal.outcome
            )
            .into());
        }
        wake.wait_after(observed);
    }
    clock.0.store(179_999_999_999, Ordering::Release);
    assert_eq!(parent.next_live_reply(clock.now()).map_err(|e| e as Box<dyn Error>)?, Some(index));
    let mut buffer = peer.control.take_write_buffer()?.ok_or("Original buffer unavailable")?;
    let count = FrameCodec::encode(
        &WireMessage {
            header: h,
            payload: WirePayload::Reply(ReplyOutcome::Failed(LifecycleFailureCode::Preparation)),
        },
        buffer.as_mut_bytes(),
    )?;
    let allocation = buffer.as_mut_bytes().as_ptr();
    let encoded: [u8; 32] = Sha256::digest(&buffer.as_mut_bytes()[..count]).into();
    let original = peer.control.original_deadline();
    clock.0.store(180_000_000_000, Ordering::Release);
    let mut rejected = match peer.control.write(
        LifecyclePipeOperationId::new(1)?,
        Moment::from_nanos(180_000_000_000),
        buffer,
        count,
    ) {
        Err(rejected) => rejected,
        Ok(_) => return Err("Expired ordinary reply admitted".into()),
    };
    assert_eq!(rejected.error, LifecyclePipeError::DeadlineExpired);
    assert_eq!(rejected.buffer.as_mut_bytes().as_ptr(), allocation);
    assert_eq!(FrameCodec::decode(&rejected.buffer.as_mut_bytes()[..count])?.header, h);
    assert!(peer.control.take_write_buffer()?.is_none());
    assert!(
        parent
            .retire_unissued_expired_write(
                WritePurpose::Reply(index),
                Moment::from_nanos(180_000_000_000),
                rejected.error,
                &rejected.buffer.as_mut_bytes()[..count],
                encoded,
                clock.now()
            )
            .map_err(|e| e as Box<dyn Error>)?
    );
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Cause discarded")?.write_cutoff(),
        Some(LifecyclePipeError::DeadlineExpired)
    );
    assert_eq!(peer.control.original_deadline(), original);
    assert!(!parent.closed && parent.failure().is_none() && !parent.native_retired());
    assert!(!parent.requests.get(index)?.sent);
    assert_eq!(
        coordinator
            .failure(lease.resource)
            .and_then(Error::source)
            .ok_or("Original cause lost")?
            .to_string(),
        "original preparation producer"
    );
    assert!(!parent.requests.get(index)?.sent);
    drop(rejected.buffer);
    let mut returned = peer.control.take_write_buffer()?.ok_or("Returned original slot lost")?;
    assert_eq!(returned.as_mut_bytes().as_ptr(), allocation);
    drop(returned);
    server.control.close();
    peer.control.close();
    drop(bootstrap);
    for session in [&mut peer, &mut server] {
        let terminal = terminal(&client, session, &wake)?;
        assert!(matches!(terminal.outcome, JobOutcome::Cancelled));
        let retirement =
            session.control.take_checked_retirement().ok_or("Actual retirement missing")?;
        assert!(retirement.matches(&session.control));
    }
    drop(peer);
    drop(server);
    drop(client);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

fn terminal(
    client: &sugarsubstitute_execution::ExecutionClient,
    session: &mut LifecyclePipeSession,
    wake: &Wake,
) -> Result<
    sugarsubstitute_contracts::execution::JobCompletion<
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeReport,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeFailure,
    >,
> {
    loop {
        let observed = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(63))?;
        if let Some(terminal) = session.completion.try_take_completion() {
            return Ok(terminal);
        }
        wake.wait_after(observed);
    }
}

#[test]
fn exact_original_progress_and_provisional_idle_rejection_retire_without_readiness_or_ack()
-> Result<()> {
    for idle in [false, true] {
        let (parent, h, lease, _) = pending()?;
        let mut parent = Box::new(parent);
        let reference = super::super::super::replies::reference(lease);
        let (header, purpose, payload) = if idle {
            complete_progress(&mut parent, h)?;
            parent.begin_output_sequence(lease)?;
            let output = WireHeader { deadline_qpc: h.deadline_qpc, ..retirement_header(h)? };
            let (index, _) = admit_output(&mut parent, output, lease, None)?;
            parent.requests.acquisition(index, h.request.ok_or("Original")?)?;
            parent.requests.get_mut(index)?.until = Some(Moment::from_nanos(180_000_000_000));
            parent.requests.get_mut(index)?.reply = Some(Reply::OutputIdle(lease));
            (output, WritePurpose::Reply(index), ReplyOutcome::OutputIdle(reference))
        } else {
            (h, WritePurpose::AcquisitionProgress(h), ReplyOutcome::Pending(reference))
        };
        let mut bytes = [0; 256];
        let count = FrameCodec::encode(
            &WireMessage { header, payload: WirePayload::Reply(payload) },
            &mut bytes,
        )?;
        let encoded: [u8; 32] = Sha256::digest(&bytes[..count]).into();
        assert!(
            parent
                .retire_unissued_expired_write(
                    purpose,
                    Moment::from_nanos(180_000_000_000),
                    LifecyclePipeError::DeadlineExpired,
                    &bytes[..count],
                    encoded,
                    Moment::from_nanos(180_000_000_000)
                )
                .map_err(|e| e as Box<dyn Error>)?
        );
        assert_eq!(
            parent.acquisition.as_ref().ok_or("Original lost")?.write_cutoff(),
            Some(LifecyclePipeError::DeadlineExpired)
        );
        assert!(!parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
        assert!(parent.lease.is_none() && parent.output.is_none());
        assert_eq!(parent.main.snapshot().identity, None);
        assert_eq!(parent.next_delivery, 0);
        assert!(!parent.native_retired() && !parent.closed);
    }
    Ok(())
}

#[test]
fn no_issue_cutoff_classifier_keeps_foreign_changed_other_deadline_and_closed_strict() -> Result<()>
{
    for fault in 0..6 {
        let (mut parent, h, lease, _) = pending()?;
        let index = parent.requests.find(h.request.ok_or("Original")?).ok_or("Slot")?;
        parent
            .acquisition
            .as_mut()
            .ok_or("Original")?
            .observe_backend(h, RequestOutcome::Failed)?;
        parent
            .acquisition
            .as_mut()
            .ok_or("Original")?
            .reply(h, Reply::Failed(LifecycleFailureCode::Preparation))?;
        parent.requests.get_mut(index)?.reply =
            Some(Reply::Failed(LifecycleFailureCode::Preparation));
        let mut frame = h;
        let mut until = Moment::from_nanos(180_000_000_000);
        let mut cause = LifecyclePipeError::DeadlineExpired;
        let mut payload = ReplyOutcome::Failed(LifecycleFailureCode::Preparation);
        match fault {
            0 => frame.connection = Some(ConnectionId::new(2)?),
            1 => payload = ReplyOutcome::Failed(LifecycleFailureCode::Deadline),
            2 => until = Moment::from_nanos(179_000_000_000),
            3 => cause = LifecyclePipeError::Closed,
            4 => parent
                .acquisition
                .as_mut()
                .ok_or("Original")?
                .observe_write(h, WriteObservation::Queued)?,
            _ => {}
        }
        let mut bytes = [0; 256];
        let count = FrameCodec::encode(
            &WireMessage { header: frame, payload: WirePayload::Reply(payload) },
            &mut bytes,
        )?;
        let mut encoded: [u8; 32] = Sha256::digest(&bytes[..count]).into();
        if fault == 5 {
            encoded[0] ^= 1;
        }
        assert!(
            !parent
                .retire_unissued_expired_write(
                    WritePurpose::Reply(index),
                    until,
                    cause,
                    &bytes[..count],
                    encoded,
                    Moment::from_nanos(180_000_000_000)
                )
                .map_err(|e| e as Box<dyn Error>)?
        );
        assert_eq!(parent.acquisition.as_ref().ok_or("Original lost")?.write_cutoff(), None);
        assert!(
            !parent.closed
                && parent
                    .acquisition
                    .as_ref()
                    .ok_or("Original lost")?
                    .output_lease(
                        copied_binding(lease),
                        super::super::super::replies::reference(lease)
                    )
                    .is_err()
        );
    }
    Ok(())
}
