// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound reads and content hashing under the same original passive source custody.
use super::*;
use sha2::{Digest, Sha256};

pub(super) fn rejected(code: PortableIncidentFailureCode) -> PortableIncidentError {
    PortableIncidentError::rejected(code)
}
pub(super) fn control(context: &JobContext) -> Result<(), PortableIncidentError> {
    if context.cancellation_requested() {
        return Err(rejected(PortableIncidentFailureCode::Cancelled));
    }
    if context.deadline().is_some_and(|until| context.now() >= until) {
        return Err(rejected(PortableIncidentFailureCode::Deadline));
    }
    Ok(())
}
pub(super) fn push<T>(
    items: &mut Vec<T>,
    bound: usize,
    value: T,
) -> Result<(), PortableIncidentError> {
    if items.len() >= bound {
        return Err(rejected(PortableIncidentFailureCode::Capacity));
    }
    items.try_reserve(1).map_err(PortableIncidentError::allocation)?;
    items.push(value);
    Ok(())
}
pub(super) fn bind(
    session: &mut dyn PortableIncidentReadSession,
    entry: PortableIncidentEntry,
    object: DiagnosticObject,
    context: &JobContext,
) -> Result<(), PortableIncidentError> {
    control(context)?;
    if !session.bind_typed_object(entry.id, object, context)? {
        return Err(rejected(PortableIncidentFailureCode::ChangedSource));
    }
    Ok(())
}
pub(super) fn record(
    session: &mut dyn PortableIncidentReadSession,
    entry: PortableIncidentEntry,
    bound: usize,
    context: &JobContext,
) -> Result<Vec<u8>, PortableIncidentError> {
    if entry.bytes > bound as u64 {
        return Err(rejected(PortableIncidentFailureCode::Capacity));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(entry.bytes as usize).map_err(PortableIncidentError::allocation)?;
    stream(session, entry, context, |chunk| {
        bytes.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(bytes)
}
pub(super) fn stream(
    session: &mut dyn PortableIncidentReadSession,
    entry: PortableIncidentEntry,
    context: &JobContext,
    mut consume: impl FnMut(&[u8]) -> Result<(), PortableIncidentError>,
) -> Result<[u8; 32], PortableIncidentError> {
    let mut scratch = [0; DIAGNOSTIC_IO_BYTES];
    let mut offset = 0;
    let mut hash = Sha256::new();
    while offset < entry.bytes {
        control(context)?;
        let bound = (entry.bytes - offset).min(DIAGNOSTIC_IO_BYTES as u64) as usize;
        let count = session.read_chunk(entry.id, offset, &mut scratch[..bound], context)?;
        if count == 0 || count > bound {
            return Err(rejected(PortableIncidentFailureCode::WrongDigest));
        }
        hash.update(&scratch[..count]);
        consume(&scratch[..count])?;
        offset += count as u64;
    }
    control(context)?;
    if session.read_chunk(entry.id, offset, &mut scratch[..1], context)? != 0 {
        return Err(rejected(PortableIncidentFailureCode::WrongDigest));
    }
    Ok(hash.finalize().into())
}
