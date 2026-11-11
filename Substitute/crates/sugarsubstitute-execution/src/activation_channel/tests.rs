// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual private pipes qualify framing, native cancellation, inheritance and checked cleanup.
use super::*;
use crate::runtime_test_support::TestResult;
mod support;
use std::os::windows::io::AsRawHandle;
use support as worker;
use windows::Win32::{
    Foundation::{
        DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, HANDLE_FLAG_PROTECT_FROM_CLOSE,
        SetHandleInformation,
    },
    System::Threading::GetCurrentProcess,
};

/// Duplicate the fixture's uniquely owned child slot, just as explicit process inheritance does.
fn endpoints(
    context: &JobContext,
) -> Result<(ChannelEndpoint, ChannelEndpoint), NativeObservationError> {
    let mut pair = PreparedChannel::prepare(context)?;
    let child = pair
        .with_child(|handle| {
            let mut raw = HANDLE::default();
            // SAFETY: exact borrowed source remains live and the unique duplicate is immediately captured.
            unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    HANDLE(handle.as_raw_handle()),
                    GetCurrentProcess(),
                    &mut raw,
                    0,
                    true,
                    DUPLICATE_SAME_ACCESS,
                )
            }
            .map_err(|cause| NativeObservationError::new("channel_fixture_duplicate", cause))?;
            // SAFETY: only this controlled fixture owns the new slot; no other owner can close or reuse it.
            unsafe { capture_inherited(raw.0 as usize) }
        })?
        .admit(context)?;
    pair.retire_child()?;
    Ok((pair.endpoint(), child))
}
/// Observe real kernel completion; no timing sleeps, retries or altered production deadlines.
fn completed(
    endpoint: &ChannelEndpoint,
    context: &JobContext,
) -> Result<ChannelProgress, NativeObservationError> {
    loop {
        match endpoint.poll(context)? {
            ChannelProgress::Pending => crate::scope::lock(&endpoint.0)
                .operation
                .as_ref()
                .ok_or_else(|| invalid("channel_fixture_operation"))?
                .wait_fixture()?,
            value => return Ok(value),
        }
    }
}
fn retired(endpoint: &ChannelEndpoint, context: &JobContext) -> Result<(), NativeObservationError> {
    while !endpoint.retire(context)? {
        crate::scope::lock(&endpoint.0)
            .operation
            .as_ref()
            .ok_or_else(|| invalid("channel_fixture_operation"))?
            .wait_fixture()?;
    }
    Ok(())
}
/// Controlled peer corruption uses the same real retained native writer, changing only wire bytes.
fn raw_send(endpoint: &ChannelEndpoint, bytes: Vec<u8>) -> Result<(), NativeObservationError> {
    let mut state = crate::scope::lock(&endpoint.0);
    state.operation = Some(Box::new(Operation::raw_peer(bytes, state.memory.clone())?));
    state.last = Some(Direction::Write);
    state.issue()
}

#[test]
fn private_request_reply_transfers_maximum_frame_and_preserves_completed_results() -> TestResult {
    worker::run_reserved(CHANNEL_RESERVATION_BYTES * 3, |context, _| {
        let (parent, child) = endpoints(context)?;
        let payload = vec![0x5a; MAX_FRAME_BYTES];
        parent.receive(context)?;
        child.send(&payload, context)?;
        let ChannelProgress::Received(frame) = completed(&parent, context)? else {
            return Err("Frame absent".into());
        };
        assert_eq!(frame.bytes(), payload);
        assert!(matches!(completed(&child, context)?, ChannelProgress::Sent));
        assert!(matches!(parent.poll(context)?, ChannelProgress::Received(_)));
        assert!(child.send(b"duplicate", context).is_err());
        assert!(parent.receive(context).is_err());
        child.receive(context)?;
        parent.send(b"creation result", context)?;
        let ChannelProgress::Received(reply) = completed(&child, context)? else {
            return Err("Reply absent".into());
        };
        assert_eq!(reply.bytes(), b"creation result");
        assert!(matches!(completed(&parent, context)?, ChannelProgress::Sent));
        retired(&parent, context)?;
        retired(&child, context)?;
        assert_eq!(frame.bytes(), payload);
        assert!(parent.poll(context).is_err());
        Ok(())
    })
}

#[test]
fn hostile_header_is_rejected_before_body_allocation_and_remains_terminal() -> TestResult {
    worker::run_reserved(CHANNEL_RESERVATION_BYTES * 3, |context, _| {
        for bytes in [
            b"SL02\x01\0\0\0".to_vec(),
            b"SL01\0\0\0\0".to_vec(),
            [b"SL01".as_slice(), &((MAX_FRAME_BYTES + 1) as u32).to_le_bytes()].concat(),
        ] {
            let (parent, child) = endpoints(context)?;
            parent.receive(context)?;
            raw_send(&child, bytes)?;
            assert!(completed(&parent, context).is_err());
            assert!(parent.poll(context).is_err());
            assert!(crate::scope::lock(&parent.0).received.is_none());
            assert!(matches!(completed(&child, context)?, ChannelProgress::Sent));
            retired(&parent, context)?;
            retired(&child, context)?;
        }
        Ok(())
    })
}

#[test]
fn truncated_frame_and_peer_exit_never_produce_a_complete_message() -> TestResult {
    worker::run_reserved(CHANNEL_RESERVATION_BYTES * 3, |context, _| {
        let (parent, child) = endpoints(context)?;
        parent.receive(context)?;
        raw_send(&child, b"SL01".to_vec())?;
        assert!(matches!(completed(&child, context)?, ChannelProgress::Sent));
        retired(&child, context)?;
        assert!(completed(&parent, context).is_err());
        assert!(crate::scope::lock(&parent.0).received.is_none());
        retired(&parent, context)?;
        Ok(())
    })
}

#[test]
fn pending_read_cancellation_waits_for_actual_native_completion_and_closes_admission() -> TestResult
{
    worker::run_reserved(CHANNEL_RESERVATION_BYTES * 3, |context, _| {
        let (parent, child) = endpoints(context)?;
        parent.receive(context)?;
        assert!(matches!(parent.poll(context)?, ChannelProgress::Pending));
        retired(&parent, context)?;
        assert!(parent.send(b"late", context).is_err());
        assert!(parent.receive(context).is_err());
        retired(&child, context)?;
        Ok(())
    })
}

#[test]
fn failed_pipe_close_retains_the_exact_handle_for_checked_cleanup_retry() -> TestResult {
    worker::run_reserved(CHANNEL_RESERVATION_BYTES * 3, |context, _| {
        let (parent, child) = endpoints(context)?;
        let raw = crate::scope::lock(&parent.0).pipe.as_ref().ok_or("Pipe absent")?.raw();
        // SAFETY: the fixture alone retains and protects this exact private endpoint.
        unsafe {
            SetHandleInformation(
                raw,
                HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                HANDLE_FLAG_PROTECT_FROM_CLOSE,
            )
        }?;
        let failed = parent.retire(context);
        // SAFETY: a failed checked close keeps this same original handle alive for restoration.
        unsafe { SetHandleInformation(raw, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, Default::default()) }?;
        assert_eq!(
            failed.err().ok_or("Protected close succeeded")?.stage(),
            "activation_channel_close"
        );
        assert!(parent.send(b"late", context).is_err());
        retired(&parent, context)?;
        retired(&child, context)?;
        Ok(())
    })
}

#[test]
fn inherited_capture_rejects_synchronous_pipes_before_any_io_is_admitted() -> TestResult {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::{Security::SECURITY_ATTRIBUTES, System::Pipes::CreatePipe};
    worker::run_reserved(CHANNEL_RESERVATION_BYTES, |_, _| {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            bInheritHandle: true.into(),
            lpSecurityDescriptor: std::ptr::null_mut(),
        };
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        // SAFETY: both actual synchronous fixture outputs are immediately adopted before checking.
        let created = unsafe { CreatePipe(&mut read, &mut write, Some(&attributes), 0) };
        // SAFETY: each non-invalid output is uniquely owned by this fixture.
        let reader = (!read.is_invalid()).then(|| unsafe { OwnedHandle::from_raw_handle(read.0) });
        // SAFETY: the distinct write output has the same exclusive ownership guarantee.
        let writer =
            (!write.is_invalid()).then(|| unsafe { OwnedHandle::from_raw_handle(write.0) });
        created?;
        assert_eq!(
            native::capture(read.0 as usize).err().ok_or("Synchronous pipe admitted")?.stage(),
            "activation_channel_synchronous_handle"
        );
        drop(reader);
        drop(writer);
        Ok(())
    })
}

#[test]
fn fresh_cleanup_scope_retires_pending_io_without_reviving_cancelled_channel_admission()
-> TestResult {
    use crate::runtime_test_support::Rig;
    use sugarsubstitute_contracts::execution::{ExecutionPort, ScopeId, ScopeSpec};
    let rig = Rig::new()?;
    let (parent, child) = worker::phase(&rig, &rig.scope, 1, |context| {
        let (parent, child) = endpoints(&context)?;
        parent.receive(&context)?;
        Ok((parent, child))
    })?;
    rig.scope.cancel();
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    worker::phase(&rig, &cleanup, 2, move |context| {
        assert!(!context.cancellation_requested());
        assert!(parent.poll(&context).is_err());
        assert!(child.send(b"late", &context).is_err());
        retired(&parent, &context)?;
        retired(&child, &context)?;
        Ok(())
    })
}

#[test]
fn a_fresh_worker_cannot_extend_the_original_failed_cleanup_cutoff() -> TestResult {
    use crate::runtime_test_support::{Rig, spec};
    use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome, Moment};
    let rig = Rig::new()?;
    let (parent, child) = worker::phase(&rig, &rig.scope, 1, |context| {
        let (parent, child) = endpoints(&context)?;
        let raw = crate::scope::lock(&parent.0).pipe.as_ref().ok_or("Pipe absent")?.raw();
        // SAFETY: the fixture exclusively protects its own endpoint to force a checked close failure.
        unsafe {
            SetHandleInformation(
                raw,
                HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                HANDLE_FLAG_PROTECT_FROM_CLOSE,
            )
        }?;
        let failed = parent.retire(&context);
        // SAFETY: the failed close retains this exact slot; restore it before any assertion can fail.
        unsafe { SetHandleInformation(raw, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, Default::default()) }?;
        assert!(failed.is_err());
        retired(&child, &context)?;
        Ok((parent, child))
    })?;
    rig.clock.advance(2_000_000_001);
    let mut request = spec(2, JobClass::InfrastructureLifecycle, CHANNEL_RESERVATION_BYTES)?;
    request.deadline = Some(Moment::from_nanos(4_000_000_000));
    let mut job = rig.client.submit_blocking(&rig.scope, request, move |context| {
        assert!(!context.cancellation_requested());
        let cause = parent
            .retire(&context)
            .err()
            .ok_or_else(|| std::io::Error::other("Cleanup deadline renewed"))?;
        assert_eq!(cause.stage(), "activation_channel_admission");
        // No I/O was issued. Dropping the now-unprotected fixture owner closes its remaining handle.
        drop(parent);
        drop(child);
        Ok::<_, std::io::Error>(())
    })?;
    match rig.completion(&mut job)?.outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause.into()),
        other => Err(format!("Unexpected cutoff fixture outcome: {:?}", other.kind()).into()),
    }
}
