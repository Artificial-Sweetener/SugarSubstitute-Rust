// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply codec effects only after the original bound run and actual peer authenticate.
use super::{Result, evidence::Case, failure, options::CaseKind, protocol};
use sugarsubstitute_contracts::{
    execution::JobOutcome,
    lifecycle::{ConnectionId, RequestId},
};
use sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeObservation;
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WirePayload,
};

pub(super) fn observe(case: &mut Case, observation: LifecyclePipeObservation) -> Result<()> {
    let control = &case.session.as_ref().ok_or("Lifecycle parent session missing")?.control;
    match observation {
        LifecyclePipeObservation::Prepared => {
            if case.prepared {
                return Err("Lifecycle preparation repeated".into());
            }
            if control.prepared_run() != Some(case.run) {
                return Err("Lifecycle prepared run changed".into());
            }
            case.prepared = true;
        }
        LifecyclePipeObservation::Connected => {
            if case.connected || case.process.is_none() {
                return Err("Lifecycle connection preceded contained enrollment/repeated".into());
            }
            case.connected = true;
            protocol::read(control, 1)?;
        }
        LifecyclePipeObservation::ReadPending { operation } => {
            if !matches!(operation.get(), 1 | 3) {
                return Err("Lifecycle parent pending-read identity differs".into());
            }
        }
        LifecyclePipeObservation::Authenticated(peer) => {
            if case.kind != CaseKind::Valid
                || !case.auth_requested
                || case.peer.is_some()
                || peer.run() != case.run
                || peer.process() != case.process
            {
                return Err("Lifecycle actual peer differs from trusted registry enrollment".into());
            }
            case.peer = Some(peer);
        }
        LifecyclePipeObservation::Written { operation, bytes } => {
            if bytes == 0 {
                return Err("Lifecycle parent observed an empty write".into());
            }
            match operation.get() {
                2 if case.welcome_sent && !case.welcome_written => case.welcome_written = true,
                4 if case.reply_sent && !case.reply_written => case.reply_written = true,
                _ => return Err("Lifecycle parent write identity/order differs".into()),
            }
        }
        LifecyclePipeObservation::Received { operation, frame } => {
            let decoded = FrameCodec::decode(frame.as_bytes())?;
            let deadline = case.bootstrap_deadline.ok_or("Lifecycle launch deadline missing")?;
            match (operation.get(), decoded.payload) {
                (1, DecodedPayload::Hello(capability)) if !case.hello => {
                    if case.kind == CaseKind::ForeignGeneration {
                        let mut expected = case.run;
                        expected.generation = expected.generation.next()?;
                        protocol::validate(
                            decoded.header,
                            protocol::header(expected, None, None, deadline.ticks()),
                        )?;
                        case.hello = true;
                        case.denied = true;
                        case.pipe_close_requested = true;
                        control.close();
                    } else {
                        protocol::validate(
                            decoded.header,
                            protocol::header(case.run, None, None, deadline.ticks()),
                        )?;
                        case.hello = true;
                        control
                            .verify_capability(*capability.bytes(), control.original_deadline())?;
                        case.auth_requested = true;
                    }
                }
                (3, DecodedPayload::Register)
                    if case.peer.is_some() && case.welcome_sent && !case.register =>
                {
                    protocol::validate(
                        decoded.header,
                        protocol::header(
                            case.run,
                            Some(ConnectionId::new(1)?),
                            Some(RequestId::new(1)?),
                            deadline.ticks(),
                        ),
                    )?;
                    case.register = true;
                }
                _ => {
                    return Err(
                        "Lifecycle parent received unexpected/unadmitted codec operation".into()
                    );
                }
            }
            drop(frame);
        }
    }
    Ok(())
}
pub(super) fn advance(case: &mut Case, closing: bool) -> Result<()> {
    let Some(session) = &case.session else { return Ok(()) };
    let control = &session.control;
    if closing {
        if !case.pipe_close_requested {
            case.pipe_close_requested = true;
            control.close();
        }
        return Ok(());
    }
    if case.peer.is_some() && !case.welcome_sent {
        let deadline = case.bootstrap_deadline.ok_or("Lifecycle launch deadline missing")?;
        protocol::write(
            control,
            2,
            protocol::message(
                protocol::header(case.run, Some(ConnectionId::new(1)?), None, deadline.ticks()),
                WirePayload::Welcome { qpc_frequency: deadline.frequency() },
            ),
        )?;
        protocol::read(control, 3)?;
        case.welcome_sent = true;
    }
    if case.register && case.welcome_written && !case.reply_sent {
        let deadline = case.bootstrap_deadline.ok_or("Lifecycle launch deadline missing")?;
        protocol::write(
            control,
            4,
            protocol::message(
                protocol::header(
                    case.run,
                    Some(ConnectionId::new(1)?),
                    Some(RequestId::new(1)?),
                    deadline.ticks(),
                ),
                WirePayload::Reply(ReplyOutcome::Acknowledged),
            ),
        )?;
        case.reply_sent = true;
    }
    // The valid child must finish its own pending-read cancellation before exit0.
    // Keep this idle server connected until actual process exit, preventing a race.
    if case.exited.is_some()
        && (case.kind != CaseKind::Valid || case.reply_written)
        && !case.pipe_close_requested
    {
        case.pipe_close_requested = true;
        control.close();
    }
    Ok(())
}
pub(super) fn consume(case: &mut Case) -> Result<()> {
    let Some(session) = &mut case.session else { return Ok(()) };
    let job = &mut session.completion;
    let expected = (job.scope_id(), job.spec());
    let Some(kind) = case.terminal_notice.kind() else { return Ok(()) };
    let Some(completion) = job.try_take_completion() else { return Ok(()) };
    case.pipe_terminal = true;
    if completion.scope_id != expected.0
        || completion.spec != expected.1
        || completion.outcome.kind() != kind
    {
        return Err("Lifecycle parent terminal identity differs".into());
    }
    match completion.outcome {
        JobOutcome::Cancelled if case.pipe_close_requested => Ok(()),
        JobOutcome::Failure(error)
            if case.kind == CaseKind::WrongCapability
                && case.hello
                && case.auth_requested
                && case.peer.is_none()
                && error.native_cause().stage() == "pipe_capability_mismatch"
                && !error.requires_process_shutdown()
                && !error.contains_owner_panic() =>
        {
            case.denied = true;
            case.expected_failure = Some(error);
            Ok(())
        }
        other => Err(Box::new(failure::unexpected(other))),
    }
}
