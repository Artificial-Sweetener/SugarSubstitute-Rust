// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight every entry byte before mutating its fixed private frame allocation.
use super::*;
use sugarsubstitute_contracts::coordinator_entry::EntryOwnerDegradation;

pub(in crate::coordinator_entry) fn encoded_length(
    header: &Header,
    payload: &Payload<'_>,
) -> Result<usize, EntryTransportError> {
    header.validate(payload.kind())?;
    if let Payload::Accepted(run) = payload
        && (run.owner.namespace != header.namespace || Some(run.owner.epoch) != header.epoch)
    {
        return Err(EntryTransportError::Authentication);
    }
    let body = body_length(payload)?;
    let total = HEADER_BYTES.checked_add(body).ok_or(EntryTransportError::Capacity)?;
    if total > MAX_ENTRY_FRAME_BYTES {
        return Err(EntryTransportError::Capacity);
    }
    Ok(total)
}
fn body_length(payload: &Payload<'_>) -> Result<usize, EntryTransportError> {
    Ok(match payload {
        Payload::Launch(request) => launch_payload::encoded_length(request)?,
        Payload::Accepted(_) => 16,
        Payload::Rejected(_) => 2,
        _ => 0,
    })
}

pub(in crate::coordinator_entry) fn encode(
    output: &mut [u8],
    header: &Header,
    payload: Payload<'_>,
) -> Result<usize, EntryTransportError> {
    let length = encoded_length(header, &payload)?;
    if output.len() < length {
        return Err(EntryTransportError::Capacity);
    }
    let mut writer = Writer::new(&mut output[..length]);
    write_header(&mut writer, header, payload.kind(), length - HEADER_BYTES)?;
    match payload {
        Payload::Launch(request) => {
            launch_payload::encode(&mut writer.output[HEADER_BYTES..], request)?;
        }
        Payload::Accepted(run) => {
            writer.bytes(&run.instance.get().to_le_bytes())?;
            writer.bytes(&run.generation.get().to_le_bytes())?;
        }
        Payload::Rejected(rejection) => writer.bytes(&rejection_bytes(rejection))?,
        _ => {}
    }
    Ok(length)
}
fn write_header(
    writer: &mut Writer<'_>,
    header: &Header,
    kind: Kind,
    body_length: usize,
) -> Result<(), EntryTransportError> {
    header.validate(kind)?;
    writer.bytes(MAGIC)?;
    writer.u16(VERSION)?;
    writer.u16(kind as u16)?;
    writer.u32(body_length)?;
    writer.u32(0)?;
    writer.bytes(header.namespace.bytes())?;
    writer.bytes(header.epoch.as_ref().map_or(&[0; 16][..], |epoch| epoch.bytes().as_slice()))?;
    writer.bytes(&header.request.get().to_le_bytes())?;
    writer.bytes(&header.source.ticks().to_le_bytes())?;
    writer.bytes(&header.nonce)?;
    writer.bytes(&header.source.frequency().to_le_bytes())?;
    Ok(())
}
pub(in crate::coordinator_entry) fn stage_launch(
    output: &mut [u8],
    request: &EntryLaunchRequest,
) -> Result<usize, EntryTransportError> {
    let length = HEADER_BYTES
        .checked_add(body_length(&Payload::Launch(request))?)
        .ok_or(EntryTransportError::Capacity)?;
    if length > MAX_ENTRY_FRAME_BYTES || output.len() < length {
        return Err(EntryTransportError::Capacity);
    }
    output[..HEADER_BYTES].fill(0);
    launch_payload::encode(&mut output[HEADER_BYTES..length], request)?;
    Ok(length)
}
pub(in crate::coordinator_entry) fn bind_staged_launch(
    output: &mut [u8],
    length: usize,
    header: &Header,
) -> Result<(), EntryTransportError> {
    header.validate(Kind::Launch)?;
    if !(HEADER_BYTES..=MAX_ENTRY_FRAME_BYTES).contains(&length) || output.len() < length {
        return Err(EntryTransportError::Capacity);
    }
    write_header(
        &mut Writer::new(&mut output[..HEADER_BYTES]),
        header,
        Kind::Launch,
        length - HEADER_BYTES,
    )
}

fn rejection_bytes(rejection: EntryRejection) -> [u8; 2] {
    match rejection {
        EntryRejection::InvalidInput => [1, 0],
        EntryRejection::ProfileBusy => [2, 0],
        EntryRejection::Capacity => [3, 0],
        EntryRejection::Deadline => [4, 0],
        EntryRejection::Authentication => [5, 0],
        EntryRejection::ChangedDuplicate => [6, 0],
        EntryRejection::RecoverableContention => [7, 0],
        EntryRejection::StaleRequest => [9, 0],
        EntryRejection::DegradedOwner(reason) => [
            8,
            match reason {
                EntryOwnerDegradation::IdentityUncertain => 1,
                EntryOwnerDegradation::RecordUnknown => 2,
                EntryOwnerDegradation::EndpointCollision => 3,
                EntryOwnerDegradation::NativeCleanupUncertain => 4,
            },
        ],
    }
}
