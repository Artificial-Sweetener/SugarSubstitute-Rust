// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode bounded borrowed reply data without constructing native trust or output custody.
use super::{
    GrantedContext, MAX_PROFILE_FOLDER_UNITS, OutputDelivery, ProtocolError, ReplyOutcome,
    VersionLabel,
    binary::{Reader, Writer},
    decode, encode, main_fields, reply,
};
use sugarsubstitute_contracts::{
    lifecycle::RunKey,
    transcript::{MAX_BATCH_BYTES, OutputGap, OutputStream, StreamSequence},
};

pub(super) fn write(
    value: ReplyOutcome<'_>,
    run: RunKey,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    match value {
        ReplyOutcome::Acknowledged => writer.u8(1),
        ReplyOutcome::Pending(lease) => {
            writer.u8(2)?;
            encode::lease(lease, writer)
        }
        ReplyOutcome::Granted(context) => {
            writer.u8(3)?;
            encode::lease(context.lease, writer)?;
            main_fields::write_identity(context.identity, run, writer)?;
            main_fields::write_origin(context.origin, writer)?;
            write_version(context.frontend, writer)?;
            write_version(context.sugarcubes, writer)
        }
        ReplyOutcome::Released => writer.u8(4),
        ReplyOutcome::Reloaded(lease) => {
            writer.u8(5)?;
            encode::lease(lease, writer)
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
        ReplyOutcome::EnvironmentAuthorized(identity) => {
            writer.u8(9)?;
            main_fields::write_identity(identity, run, writer)
        }
        ReplyOutcome::BrowserAuthorized { identity, browser_process_id } => {
            writer.u8(10)?;
            main_fields::write_identity(identity, run, writer)?;
            writer.u32(browser_process_id.get())
        }
        ReplyOutcome::FolderChunkAccepted { identity, next_offset_units } => {
            check_offset(next_offset_units)?;
            writer.u8(11)?;
            main_fields::write_identity(identity, run, writer)?;
            writer.u32(next_offset_units)
        }
        ReplyOutcome::OutputDelivery(delivery) => {
            writer.u8(12)?;
            write_output(delivery, writer)
        }
        ReplyOutcome::OutputIdle(lease) => {
            writer.u8(13)?;
            encode::lease(lease, writer)
        }
        ReplyOutcome::MainStageFailed(value) => {
            writer.u8(14)?;
            main_fields::write_stage_failure(value, run, writer)
        }
        ReplyOutcome::BackendUnavailable { failed, status, affected_clients, retry_ready } => {
            check_affected_clients(affected_clients)?;
            writer.u8(15)?;
            encode::lease(failed, writer)?;
            writer.u32(status)?;
            writer.u16(affected_clients)?;
            writer.u8(u8::from(retry_ready))
        }
        ReplyOutcome::BackendReplaced { identity, previous, current } => {
            writer.u8(16)?;
            main_fields::write_identity(identity, run, writer)?;
            encode::lease(previous, writer)?;
            encode::lease(current, writer)
        }
    }
}
pub(super) fn read<'a>(
    input: &mut Reader<'a>,
    run: RunKey,
) -> Result<ReplyOutcome<'a>, ProtocolError> {
    Ok(match input.u8()? {
        1 => ReplyOutcome::Acknowledged,
        2 => ReplyOutcome::Pending(decode::lease(input)?),
        3 => ReplyOutcome::Granted(GrantedContext {
            lease: decode::lease(input)?,
            identity: main_fields::read_identity(input, run)?,
            origin: main_fields::read_origin(input)?,
            frontend: read_version(input)?,
            sugarcubes: read_version(input)?,
        }),
        4 => ReplyOutcome::Released,
        5 => ReplyOutcome::Reloaded(decode::lease(input)?),
        6 => ReplyOutcome::Cancelled,
        7 => ReplyOutcome::Failed(reply::failure(input.u8()?)?),
        8 => ReplyOutcome::Rejected(reply::rejection(input.u8()?)?),
        9 => ReplyOutcome::EnvironmentAuthorized(main_fields::read_identity(input, run)?),
        10 => ReplyOutcome::BrowserAuthorized {
            identity: main_fields::read_identity(input, run)?,
            browser_process_id: main_fields::read_pid(input)?,
        },
        11 => {
            let identity = main_fields::read_identity(input, run)?;
            let next_offset_units = input.u32()?;
            check_offset(next_offset_units)?;
            ReplyOutcome::FolderChunkAccepted { identity, next_offset_units }
        }
        12 => ReplyOutcome::OutputDelivery(read_output(input)?),
        13 => ReplyOutcome::OutputIdle(decode::lease(input)?),
        14 => ReplyOutcome::MainStageFailed(main_fields::read_stage_failure(input, run)?),
        15 => {
            let failed = decode::lease(input)?;
            let status = input.u32()?;
            let affected_clients = input.u16()?;
            check_affected_clients(affected_clients)?;
            let retry_ready = match input.u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::UnsupportedTag),
            };
            ReplyOutcome::BackendUnavailable { failed, status, affected_clients, retry_ready }
        }
        16 => ReplyOutcome::BackendReplaced {
            identity: main_fields::read_identity(input, run)?,
            previous: decode::lease(input)?,
            current: decode::lease(input)?,
        },
        _ => return Err(ProtocolError::UnsupportedTag),
    })
}
/// Validate the closed Main cohort count before any caller-owned output mutation.
fn check_affected_clients(count: u16) -> Result<(), ProtocolError> {
    if !(1..=64).contains(&count) {
        return Err(ProtocolError::Length);
    }
    Ok(())
}
fn check_offset(offset: u32) -> Result<(), ProtocolError> {
    if offset == 0 || offset > MAX_PROFILE_FOLDER_UNITS as u32 {
        return Err(ProtocolError::FragmentShape);
    }
    Ok(())
}
fn write_version(value: VersionLabel<'_>, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
    writer.u8(value.as_str().len() as u8)?;
    writer.put(value.as_str().as_bytes())
}
fn read_version<'a>(input: &mut Reader<'a>) -> Result<VersionLabel<'a>, ProtocolError> {
    let count = usize::from(input.u8()?);
    if !(1..=64).contains(&count) {
        return Err(ProtocolError::VersionLabel);
    }
    let value = std::str::from_utf8(input.take(count)?).map_err(|_| ProtocolError::VersionLabel)?;
    VersionLabel::new(value)
}
fn write_output(value: OutputDelivery<'_>, writer: &mut Writer<'_>) -> Result<(), ProtocolError> {
    value.validate()?;
    encode::lease(value.lease, writer)?;
    writer.u64(value.delivery_id.get())?;
    writer.u8(match value.stream {
        OutputStream::Stdout => 1,
        OutputStream::Stderr => 2,
    })?;
    writer.u64(value.sequence.get())?;
    writer.u8(u8::from(value.gap.is_some()))?;
    if let Some(gap) = value.gap {
        writer.u64(gap.dropped_bytes())?;
        writer.u64(gap.skipped_batches())?;
    }
    writer.u8(u8::from(value.end))?;
    writer.u32(value.bytes.len() as u32)?;
    writer.put(value.bytes)
}
fn read_output<'a>(input: &mut Reader<'a>) -> Result<OutputDelivery<'a>, ProtocolError> {
    let lease = decode::lease(input)?;
    let delivery_id = main_fields::read_positive(input)?;
    let stream = match input.u8()? {
        1 => OutputStream::Stdout,
        2 => OutputStream::Stderr,
        _ => return Err(ProtocolError::UnsupportedTag),
    };
    let sequence = StreamSequence::new(input.u64()?).map_err(|_| ProtocolError::Identity)?;
    let gap = match input.u8()? {
        0 => None,
        1 => Some(
            OutputGap::new(input.u64()?, input.u64()?).map_err(|_| ProtocolError::OutputShape)?,
        ),
        _ => return Err(ProtocolError::OutputShape),
    };
    let end = match input.u8()? {
        0 => false,
        1 => true,
        _ => return Err(ProtocolError::OutputShape),
    };
    let count = usize::try_from(input.u32()?).map_err(|_| ProtocolError::OutputShape)?;
    if count > MAX_BATCH_BYTES {
        return Err(ProtocolError::OutputShape);
    }
    let bytes = input.take(count)?;
    Ok(OutputDelivery { lease, delivery_id, stream, sequence, gap, end, bytes })
}
