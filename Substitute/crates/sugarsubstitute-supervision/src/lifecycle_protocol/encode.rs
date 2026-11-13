// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight the entire immutable input before mutating caller-owned output.
use super::{
    HEADER_BYTES, LeaseReference, MAX_FRAME_BYTES, ProtocolError, WIRE_VERSION, WireMessage,
    WirePayload, binary::Writer, main_fields, reply_codec,
};
use sugarsubstitute_contracts::{
    configuration::{LoopbackHost, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_TOTAL_ARGUMENT_BYTES},
    lifecycle::{CompletionIntent, RunKey, UiWatchdogActivity},
};

pub(super) fn frame(message: &WireMessage<'_>, output: &mut [u8]) -> Result<usize, ProtocolError> {
    let kind = message.payload.kind();
    message.header.validate(kind)?;
    let mut measured = Writer::measure();
    payload(&message.payload, message.header.run, &mut measured)?;
    let length = HEADER_BYTES.checked_add(measured.position).ok_or(ProtocolError::Length)?;
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    if output.len() < length {
        return Err(ProtocolError::OutputTooSmall { required: length, available: output.len() });
    }
    let mut writer = Writer { output: Some(&mut output[..length]), position: 0 };
    writer.put(b"SSLC")?;
    writer.u16(WIRE_VERSION)?;
    writer.u16(HEADER_BYTES as u16)?;
    writer.u32(length as u32)?;
    writer.u16(kind)?;
    writer.u16(0)?;
    let header = message.header;
    writer.put(header.run.owner.namespace.bytes())?;
    writer.put(header.run.owner.epoch.bytes())?;
    writer.u64(header.run.instance.get())?;
    writer.u64(header.run.generation.get())?;
    writer.u64(header.connection.map_or(0, |id| id.get()))?;
    writer.u64(header.request.map_or(0, |id| id.get()))?;
    writer.put(&header.deadline_qpc.to_le_bytes())?;
    payload(&message.payload, message.header.run, &mut writer)?;
    Ok(length)
}
fn payload(
    payload: &WirePayload<'_>,
    run: RunKey,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    match payload {
        WirePayload::Hello(secret) => writer.put(secret.bytes()),
        WirePayload::Welcome { qpc_frequency } => {
            if *qpc_frequency <= 0 {
                return Err(ProtocolError::Clock);
            }
            writer.put(&qpc_frequency.to_le_bytes())
        }
        WirePayload::Register => Ok(()),
        WirePayload::Acquire { identity, target } => {
            main_fields::write_identity(*identity, run, writer)?;
            encode_target(target, writer)
        }
        WirePayload::Release(reference) => lease(*reference, writer),
        WirePayload::Reload { lease: reference, identity } => {
            lease(*reference, writer)?;
            main_fields::write_identity(*identity, run, writer)
        }
        WirePayload::ExitIntent { shutdown, intent, expected_code } => {
            writer.u64(shutdown.get())?;
            writer.u8(match intent {
                CompletionIntent::Normal => 1,
                CompletionIntent::StartupCancelled => 2,
                CompletionIntent::StartupFailed => 3,
            })?;
            writer.u32(*expected_code)
        }
        WirePayload::ExitCompleted(shutdown) => writer.u64(shutdown.get()),
        WirePayload::UiChallenge(challenge) | WirePayload::UiResponse(challenge) => {
            writer.u64(challenge.get())
        }
        WirePayload::Reply(reply) => reply_codec::write(*reply, run, writer),
        WirePayload::RequestedProfileFolder(fragment)
        | WirePayload::EnvironmentPrepared(fragment) => fragment.write(run, writer),
        WirePayload::ControllerPrepared { identity, browser_process_id } => {
            main_fields::write_identity(*identity, run, writer)?;
            writer.u32(browser_process_id.get())
        }
        WirePayload::MainRetired(identity) | WirePayload::MainRevealed(identity) => {
            main_fields::write_identity(*identity, run, writer)
        }
        WirePayload::NativeActivity { revision, activity } => {
            writer.u64(revision.get())?;
            writer.u8(match activity {
                UiWatchdogActivity::VisibleActive => 1,
                UiWatchdogActivity::Inactive => 2,
                UiWatchdogActivity::Suspended => 3,
            })
        }
        WirePayload::OutputPull { lease: reference, acknowledged_delivery } => {
            lease(*reference, writer)?;
            writer.u64(acknowledged_delivery.map_or(0, std::num::NonZeroU64::get))
        }
        WirePayload::MainFailed(value) => main_fields::write_stage_failure(*value, run, writer),
        WirePayload::SettleAttempt { identity, acquisition } => {
            main_fields::write_identity(*identity, run, writer)?;
            writer.u64(acquisition.get())
        }
        WirePayload::RetryBackend { identity, failed } => {
            main_fields::write_identity(*identity, run, writer)?;
            lease(*failed, writer)
        }
        WirePayload::UpdateStatus { revision, workflow, status } => {
            if status.len() > super::MAX_UPDATE_STATUS_BYTES {
                return Err(ProtocolError::Length);
            }
            writer.u64(revision.get())?;
            writer.u8(match workflow {
                super::WorkflowQueue::Idle => 1,
                super::WorkflowQueue::Busy => 2,
                super::WorkflowQueue::Unobserved => 3,
            })?;
            writer.u32(u32::try_from(status.len()).map_err(|_| ProtocolError::Length)?)?;
            writer.put(status)
        }
        WirePayload::RestartToApply => Ok(()),
        WirePayload::PageFailed(failure) => writer.u8(match failure {
            super::PageFailure::Unavailable => 1,
            super::PageFailure::SetupFailed => 2,
        }),
    }
}
fn encode_target(
    target: &super::BorrowedBackendTarget<'_>,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    match target {
        super::BorrowedBackendTarget::Managed(target) => {
            if target.arguments().len() > MAX_ARGUMENTS {
                return Err(ProtocolError::Length);
            }
            let mut total = 0usize;
            for argument in target.arguments() {
                if argument.len() > MAX_ARGUMENT_BYTES {
                    return Err(ProtocolError::Length);
                }
                total = total.checked_add(argument.len()).ok_or(ProtocolError::Length)?;
                if total > MAX_TOTAL_ARGUMENT_BYTES {
                    return Err(ProtocolError::Length);
                }
            }
            let root = target.root().to_str().ok_or(ProtocolError::InvalidUtf16)?;
            let interpreter = target.interpreter().to_str().ok_or(ProtocolError::InvalidUtf16)?;
            writer.u8(1)?;
            writer.u16(target.port().get())?;
            writer.u8(target.arguments().len() as u8)?;
            writer.text(root, 32766)?;
            writer.text(interpreter, 32766)?;
            for argument in target.arguments() {
                writer.text(argument, MAX_ARGUMENT_BYTES)?;
            }
            Ok(())
        }
        super::BorrowedBackendTarget::Attached(target) => {
            writer.u8(2)?;
            writer.u8(match target.host() {
                LoopbackHost::Ipv4 => 1,
                LoopbackHost::Ipv6 => 2,
                LoopbackHost::Localhost => 3,
            })?;
            writer.u16(target.port().get())
        }
    }
}
pub(super) fn lease(
    reference: LeaseReference,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    writer.u64(reference.resource.get())?;
    writer.u64(reference.generation.get())?;
    writer.u64(reference.lease.get())
}
