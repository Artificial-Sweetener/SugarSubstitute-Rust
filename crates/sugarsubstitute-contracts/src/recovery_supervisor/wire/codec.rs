// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode exact borrowed fields and reject foreign-purpose or noncanonical payloads.
use super::*;

use super::{
    fields::read32,
    payload::{self, shape},
};
use key::{array, number, put};
/// Encode one complete schema1 recovery message directly into an admitted native buffer.
/// # Errors
/// Rejects wrong direction, zero sequence, invalid original bounds/ACK or insufficient capacity.
pub fn encode(
    frame: &RecoveryFrame<'_>,
    receiver: RecoveryReceiver,
    output: &mut [u8],
) -> Result<usize, RecoveryWireError> {
    let (kind, length) = shape(&frame.command)?;
    direction(kind, receiver)?;
    if let RecoveryCommand::Commit(ack) = &frame.command
        && (ack.incident() != frame.key.report().incident()
            || frame.key.report().revision().next().map_err(|_| RecoveryWireError::Binding)?
                != ack.revision())
    {
        return Err(RecoveryWireError::Binding);
    }
    let body = header(frame.key, frame.sequence, kind, length, output)?;
    payload::write(&frame.command, body);
    Ok(RECOVERY_HEADER_BYTES + length)
}
/// Encode one original Root path fragment directly into the existing charged pipe buffer.
/// Input units must contain exactly the requested next fragment; no path copy is allocated.
/// # Errors
/// Rejects zero/malformed identity, size/offset/sequence, NUL units or changed iterator extent.
pub fn encode_capture_root_units(
    key: RecoveryGrantKey,
    sequence: u64,
    identity: [u8; 24],
    total_units: u32,
    offset_units: u32,
    mut units: impl Iterator<Item = u16>,
    output: &mut [u8],
) -> Result<(usize, u32), RecoveryWireError> {
    let total = total_units as usize;
    let offset = offset_units as usize;
    if identity == [0; 24] || total == 0 || total > RECOVERY_ROOT_BYTES / 2 || offset >= total {
        return Err(RecoveryWireError::Binding);
    }
    let count = (total - offset).min(RECOVERY_ROOT_CHUNK / 2);
    let length = 32 + count * 2;
    let body = header(key, sequence, 14, length, output)?;
    body[..24].copy_from_slice(&identity);
    body[24..28].copy_from_slice(&(total_units * 2).to_le_bytes());
    body[28..32].copy_from_slice(&(offset_units * 2).to_le_bytes());
    for at in 0..count {
        let unit = units.next().filter(|unit| *unit != 0).ok_or(RecoveryWireError::Malformed)?;
        body[32 + at * 2..34 + at * 2].copy_from_slice(&unit.to_le_bytes());
    }
    if units.next().is_some() {
        return Err(RecoveryWireError::Malformed);
    }
    Ok((RECOVERY_HEADER_BYTES + length, offset_units + count as u32))
}
fn header(
    key: RecoveryGrantKey,
    sequence: u64,
    kind: u32,
    length: usize,
    output: &mut [u8],
) -> Result<&mut [u8], RecoveryWireError> {
    let total = RECOVERY_HEADER_BYTES.checked_add(length).ok_or(RecoveryWireError::Capacity)?;
    if sequence == 0 || total > RECOVERY_WIRE_BYTES || output.len() < total {
        return Err(RecoveryWireError::Capacity);
    }
    let out = &mut output[..total];
    out.fill(0);
    out[..4].copy_from_slice(b"SSRS");
    out[4..6].copy_from_slice(&1u16.to_le_bytes());
    out[6..8].copy_from_slice(&(RECOVERY_HEADER_BYTES as u16).to_le_bytes());
    out[8..12].copy_from_slice(&kind.to_le_bytes());
    out[12..16].copy_from_slice(&(length as u32).to_le_bytes());
    put(out, 16, sequence);
    key::encode(key, &mut out[24..184])?;
    Ok(&mut out[RECOVERY_HEADER_BYTES..])
}
/// Borrow exactly one canonical recovery frame for its actual authenticated receiving role.
/// # Errors
/// Unknown schemas/commands, reserved fields, changed extents/identities and wrong directions reject.
pub fn decode(
    bytes: &[u8],
    receiver: RecoveryReceiver,
) -> Result<RecoveryFrame<'_>, RecoveryWireError> {
    if bytes.len() < RECOVERY_HEADER_BYTES
        || bytes.len() > RECOVERY_WIRE_BYTES
        || bytes[..4] != *b"SSRS"
        || array::<2>(bytes, 4)? != 1u16.to_le_bytes()
        || array::<2>(bytes, 6)? != (RECOVERY_HEADER_BYTES as u16).to_le_bytes()
        || bytes[184..192] != [0; 8]
    {
        return Err(RecoveryWireError::Malformed);
    }
    let kind = read32(bytes, 8)?;
    direction(kind, receiver)?;
    let length = read32(bytes, 12)? as usize;
    if length != bytes.len() - RECOVERY_HEADER_BYTES || number(bytes, 16)? == 0 {
        return Err(RecoveryWireError::Malformed);
    }
    let key = key::decode(&bytes[24..184])?;
    let body = &bytes[RECOVERY_HEADER_BYTES..];
    let command = payload::read(kind, key, body)?;
    Ok(RecoveryFrame { key, sequence: number(bytes, 16)?, command })
}
fn direction(kind: u32, receiver: RecoveryReceiver) -> Result<(), RecoveryWireError> {
    let valid = match kind {
        2 | 3 | 11 | 14 | 16 | 17 => receiver == RecoveryReceiver::Supervisor,
        1 | 4 | 5 | 6 | 8 | 12 => receiver == RecoveryReceiver::Coordinator,
        13 => true,
        _ => return Err(RecoveryWireError::Malformed),
    };
    if valid { Ok(()) } else { Err(RecoveryWireError::Direction) }
}
