// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight the entire immutable input before mutating caller-owned output.
use super::{
    HEADER_BYTES, LeaseReference, MAX_FRAME_BYTES, ProtocolError, ReplyOutcome, WIRE_VERSION,
    WireMessage, WirePayload, reply,
};
use sugarsubstitute_contracts::{
    configuration::{LoopbackHost, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_TOTAL_ARGUMENT_BYTES},
    lifecycle::{BackendTarget, CompletionIntent},
};

pub(super) fn frame(message: &WireMessage<'_>, output: &mut [u8]) -> Result<usize, ProtocolError> {
    let kind = message.payload.kind();
    message.header.validate(kind)?;
    let mut measured = Writer::measure();
    payload(&message.payload, &mut measured)?;
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
    payload(&message.payload, &mut writer)?;
    Ok(length)
}
struct Writer<'a> {
    output: Option<&'a mut [u8]>,
    position: usize,
}
impl Writer<'_> {
    fn measure() -> Self {
        Self { output: None, position: 0 }
    }
    fn put(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        let end = self.position.checked_add(bytes.len()).ok_or(ProtocolError::Length)?;
        if end > MAX_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        if let Some(output) = &mut self.output {
            let destination = output.get_mut(self.position..end).ok_or(ProtocolError::Length)?;
            destination.copy_from_slice(bytes);
        }
        self.position = end;
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<(), ProtocolError> {
        self.put(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), ProtocolError> {
        self.put(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), ProtocolError> {
        self.put(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), ProtocolError> {
        self.put(&value.to_le_bytes())
    }
    fn text(&mut self, value: &str, maximum_units: usize) -> Result<(), ProtocolError> {
        let units = value.encode_utf16().count();
        if units > maximum_units {
            return Err(ProtocolError::Length);
        }
        if value.contains('\0') {
            return Err(ProtocolError::InteriorNul);
        }
        self.u32(u32::try_from(units).map_err(|_| ProtocolError::Length)?)?;
        for unit in value.encode_utf16() {
            self.u16(unit)?;
        }
        Ok(())
    }
}
fn payload(payload: &WirePayload<'_>, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
    match payload {
        WirePayload::Hello(secret) => writer.put(secret.bytes()),
        WirePayload::Welcome { qpc_frequency } => {
            if *qpc_frequency <= 0 {
                return Err(ProtocolError::Clock);
            }
            writer.put(&qpc_frequency.to_le_bytes())
        }
        WirePayload::Register => Ok(()),
        WirePayload::Acquire(target) => encode_target(target, writer),
        WirePayload::Release(reference) | WirePayload::Reload(reference) => {
            lease(*reference, writer)
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
        WirePayload::Reply(reply) => outcome(*reply, writer),
    }
}
fn encode_target(target: &BackendTarget, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
    match target {
        BackendTarget::Managed(target) => {
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
        BackendTarget::Attached(target) => {
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
fn lease(reference: LeaseReference, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
    writer.u64(reference.resource.get())?;
    writer.u64(reference.generation.get())?;
    writer.u64(reference.lease.get())
}
fn outcome(outcome: ReplyOutcome, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
    match outcome {
        ReplyOutcome::Acknowledged => writer.u8(1),
        ReplyOutcome::Pending(reference) => {
            writer.u8(2)?;
            lease(reference, writer)
        }
        ReplyOutcome::Granted(reference) => {
            writer.u8(3)?;
            lease(reference, writer)
        }
        ReplyOutcome::Released => writer.u8(4),
        ReplyOutcome::Reloaded(reference) => {
            writer.u8(5)?;
            lease(reference, writer)
        }
        ReplyOutcome::Cancelled => writer.u8(6),
        ReplyOutcome::Failed(code) => {
            writer.u8(7)?;
            writer.u8(reply::failure_tag(code))
        }
        ReplyOutcome::Rejected(code) => {
            writer.u8(8)?;
            writer.u8(reply::rejection_tag(code))
        }
    }
}
