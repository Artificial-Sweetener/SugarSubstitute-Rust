// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonical schema2 binary framing with atomic encoder capacity preflight.
use super::{identity, reader::Reader, writer::Writer, *};
use DiagnosticWireError as E;
use sugarsubstitute_contracts::incident::*;
/// Encode into existing charged caller storage without allocation.
/// # Errors
/// Rejects role/context/shape/capacity before modifying any output byte.
pub fn encode(frame: DiagnosticFrame<'_>, output: &mut [u8]) -> Result<usize, E> {
    validation::check(&frame)?;
    let mut header = [0u8; MAX_HEADER_BYTES];
    let mut w = Writer::new(&mut header);
    w.put(b"SSDI")?;
    w.u8(2)?;
    w.u8(0)?;
    let c = frame.context;
    let flags = u8::from(c.operation.is_some())
        | u8::from(c.incident.is_some()) << 1
        | u8::from(c.deadline.is_some()) << 2
        | u8::from(c.at.is_some()) << 3;
    w.u8(flags)?;
    w.u8(0)?;
    w.u16(0)?;
    w.u32(0)?;
    identity::key(&mut w, frame.key)?;
    w.u64(frame.request.get())?;
    if let Some(op) = c.operation {
        identity::owner(&mut w, op.owner())?;
        w.u64(op.id().get())?
    }
    if let Some(i) = c.incident {
        identity::incident(&mut w, i.key)?;
        w.u64(i.revision.get())?
    }
    if let Some(d) = c.deadline {
        w.i64(d.ticks())?;
        w.i64(d.frequency())?
    }
    if let Some(at) = c.at {
        w.u64(at.unix_seconds())?
    }
    let (code, payload) = command_codec::write(frame.command, &mut w)?;
    let length = w.length();
    let total = length.checked_add(payload.len()).ok_or(E::Capacity)?;
    if total > MAX_FRAME_BYTES || payload.len() > MAX_CHUNK_BYTES || total > output.len() {
        return Err(E::Capacity);
    }
    header[5] = code;
    header[8..10].copy_from_slice(&u16::try_from(length).map_err(|_| E::Capacity)?.to_le_bytes());
    header[10..14]
        .copy_from_slice(&u32::try_from(payload.len()).map_err(|_| E::Capacity)?.to_le_bytes());
    output[..length].copy_from_slice(&header[..length]);
    output[length..total].copy_from_slice(payload);
    Ok(total)
}
/// Decode one complete borrowed frame, retaining no copy or native authority.
/// # Errors
/// Rejects unsupported schema/kind, reserved/trailing fields, wrong roles and bounds.
pub fn decode(bytes: &[u8]) -> Result<DiagnosticFrame<'_>, E> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(E::Capacity);
    }
    let mut prefix = Reader::new(bytes);
    if prefix.array::<4>()? != *b"SSDI" || prefix.u8()? != 2 {
        return Err(E::Schema);
    }
    let code = prefix.u8()?;
    let flags = prefix.u8()?;
    if flags & !15 != 0 || prefix.u8()? != 0 {
        return Err(E::Schema);
    }
    let header = usize::from(prefix.u16()?);
    let payload = usize::try_from(prefix.u32()?).map_err(|_| E::Capacity)?;
    if !(14..=MAX_HEADER_BYTES).contains(&header) || payload > MAX_CHUNK_BYTES {
        return Err(E::Capacity);
    }
    if header.checked_add(payload) != Some(bytes.len()) {
        return Err(E::Shape);
    }
    let mut r = Reader::new(bytes.get(14..header).ok_or(E::Shape)?);
    let key = identity::read_key(&mut r)?;
    let request = DiagnosticRequestId::new(r.u64()?)?;
    let operation = if flags & 1 != 0 {
        Some(LogicalStoreOperationKey::new(
            identity::read_owner(&mut r)?,
            IncidentOperationId::new(r.u64()?).map_err(|_| E::Identity)?,
        ))
    } else {
        None
    };
    let incident = if flags & 2 != 0 {
        Some(DiagnosticIncident {
            key: identity::read_incident(&mut r)?,
            revision: IncidentRevision::new(r.u64()?).map_err(|_| E::Identity)?,
        })
    } else {
        None
    };
    let deadline =
        if flags & 4 != 0 { Some(DiagnosticDeadline::new(r.i64()?, r.i64()?)?) } else { None };
    let at = if flags & 8 != 0 {
        Some(IncidentUtc::from_unix_seconds(r.i64()?).map_err(|_| E::Time)?)
    } else {
        None
    };
    let context = DiagnosticContext { operation, incident, deadline, at };
    let command = command_codec::read(code, &mut r, &bytes[header..], context)?;
    r.finish()?;
    let frame = DiagnosticFrame { key, request, context, command };
    validation::check(&frame)?;
    Ok(frame)
}
