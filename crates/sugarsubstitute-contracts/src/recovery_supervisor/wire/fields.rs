// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact primitive body fields before any output mutation or command acceptance.
use super::*;
use key::{array, number, put};
pub(super) fn chunk(
    total: u32,
    offset: u32,
    length: usize,
    maximum: usize,
    piece: usize,
) -> Result<(), RecoveryWireError> {
    if total == 0
        || total as usize > maximum
        || length == 0
        || length > piece
        || (offset as usize).checked_add(length).is_none_or(|end| end > total as usize)
    {
        Err(RecoveryWireError::Capacity)
    } else {
        Ok(())
    }
}
pub(super) fn read32(bytes: &[u8], at: usize) -> Result<u32, RecoveryWireError> {
    Ok(u32::from_le_bytes(array(bytes, at)?))
}
pub(super) fn write32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
pub(super) fn process(out: &mut [u8], value: RecoveryProcessIdentity) {
    write32(out, 0, value.pid());
    put(out, 8, value.birth());
}
pub(super) fn read_process(bytes: &[u8]) -> Result<RecoveryProcessIdentity, RecoveryWireError> {
    if array::<4>(bytes, 4)? != [0; 4] {
        return Err(RecoveryWireError::Malformed);
    }
    RecoveryProcessIdentity::new(read32(bytes, 0)?, number(bytes, 8)?)
        .map_err(|_| RecoveryWireError::Binding)
}
pub(super) fn failure(code: RecoveryFailureCode) -> u32 {
    match code {
        RecoveryFailureCode::Cancelled => 1,
        RecoveryFailureCode::Deadline => 2,
        RecoveryFailureCode::Launch => 3,
        RecoveryFailureCode::Authentication => 4,
        RecoveryFailureCode::Capture => 5,
        RecoveryFailureCode::Desktop => 6,
        RecoveryFailureCode::Transport => 7,
        RecoveryFailureCode::Retirement => 8,
    }
}
pub(super) fn read_failure(code: u32) -> Result<RecoveryFailureCode, RecoveryWireError> {
    Ok(match code {
        1 => RecoveryFailureCode::Cancelled,
        2 => RecoveryFailureCode::Deadline,
        3 => RecoveryFailureCode::Launch,
        4 => RecoveryFailureCode::Authentication,
        5 => RecoveryFailureCode::Capture,
        6 => RecoveryFailureCode::Desktop,
        7 => RecoveryFailureCode::Transport,
        8 => RecoveryFailureCode::Retirement,
        _ => return Err(RecoveryWireError::Malformed),
    })
}

pub(super) fn root_chunk(
    identity: [u8; 24],
    total: u32,
    offset: u32,
    units: &[u8],
) -> Result<(), RecoveryWireError> {
    chunk(total, offset, units.len(), RECOVERY_ROOT_BYTES, RECOVERY_ROOT_CHUNK)?;
    if identity == [0; 24]
        || !total.is_multiple_of(2)
        || !offset.is_multiple_of(2)
        || !units.len().is_multiple_of(2)
        || units.chunks_exact(2).any(|unit| unit == [0, 0])
    {
        return Err(RecoveryWireError::Malformed);
    }
    Ok(())
}

pub(super) fn counter(out: &mut [u8], value: CounterDeadline) {
    put(out, 0, value.ticks() as u64);
    put(out, 8, value.frequency() as u64);
}
pub(super) fn read_counter(bytes: &[u8]) -> Result<CounterDeadline, RecoveryWireError> {
    let ticks = i64::try_from(number(bytes, 0)?).map_err(|_| RecoveryWireError::Malformed)?;
    let frequency = i64::try_from(number(bytes, 8)?).map_err(|_| RecoveryWireError::Malformed)?;
    CounterDeadline::new(ticks, frequency).map_err(|_| RecoveryWireError::Malformed)
}
