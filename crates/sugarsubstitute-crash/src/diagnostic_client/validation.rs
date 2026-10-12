// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate closed role/context/payload shapes before encoding or route admission.
use super::*;
use sugarsubstitute_contracts::incident::*;
pub(super) fn check(frame: &DiagnosticFrame<'_>) -> Result<(), DiagnosticWireError> {
    use DiagnosticCommand as C;
    use DiagnosticWireError as E;
    let c = frame.command;
    let code = match c {
        C::Result { kind, .. } => kind.code(),
        _ => command_codec::code(c),
    };
    let reporter = (17..=30).contains(&code);
    let allowed = match code {
        16 => true,
        14 => frame.key.role() == DiagnosticRole::Symbolize,
        13 => matches!(frame.key.role(), DiagnosticRole::Storage | DiagnosticRole::Symbolize),
        _ => {
            frame.key.role()
                == if reporter { DiagnosticRole::Reporter } else { DiagnosticRole::Storage }
        }
    };
    if !allowed {
        return Err(E::Role);
    }
    let operation = !reporter && code != 16;
    let incident = !matches!(code, 1 | 2 | 16);
    let deadline = !(21..=27).contains(&code);
    let utc = operation;
    let context = frame.context;
    if context.operation.is_some() != operation
        || context.incident.is_some() != incident
        || context.deadline.is_some() != deadline
        || context.at.is_some() != utc
    {
        return Err(E::Shape);
    }
    if context.operation.is_some_and(|op| op.owner() != frame.key.owner())
        || context
            .incident
            .is_some_and(|i| i.key.subject().coordinator().namespace != frame.key.owner().namespace)
    {
        return Err(E::Identity);
    }
    match c {
        C::Result { reply: DiagnosticReply::Completed { bytes, .. }, .. }
            if bytes > 268_435_456 =>
        {
            return Err(E::Capacity);
        }
        C::Seed(bytes) => seed(bytes)?,
        C::Publish(DiagnosticPublication::Begin { expected, demand, seed: bytes }) => {
            seed(bytes)?;
            match expected {
                DiagnosticExpectedSummary::Absent if demand.summary_slots() == 1 => {}
                DiagnosticExpectedSummary::Exact { revision, .. }
                    if demand.summary_slots() == 0
                        && context.incident.is_some_and(|i| i.revision == revision) => {}
                _ => return Err(E::Shape),
            }
        }
        C::Publish(DiagnosticPublication::Chunk(chunk)) => {
            if chunk.kind == DiagnosticTransferKind::View {
                return Err(E::Shape);
            }
            check_chunk(chunk)?
        }
        C::Publish(DiagnosticPublication::Finish { bytes, .. }) => {
            if bytes == 0 || bytes > 65_536 {
                return Err(E::Capacity);
            }
        }
        C::ExportChunk(chunk) => check_chunk(chunk)?,
        C::RetainExport(choice) | C::SaveSelectionReady { selection: choice, .. } => {
            selection(choice)?
        }
        C::BeginView { total, .. } | C::ReplaceView { total, .. } => {
            if total == 0 || total > 262_144 {
                return Err(E::Capacity);
            }
        }
        C::ViewChunk { chunk, .. } => {
            if chunk.kind != DiagnosticTransferKind::View {
                return Err(E::Shape);
            }
            check_chunk(chunk)?
        }
        C::CloseAfterDurableAck { acknowledgement, .. } => {
            if !context.incident.is_some_and(|i| {
                i.key == acknowledgement.incident() && i.revision == acknowledgement.revision()
            }) {
                return Err(E::Identity);
            }
        }
        _ => {}
    }
    if let C::ReplaceView { old, .. } = c
        && context.incident.is_none_or(|i| old >= i.revision)
    {
        return Err(E::Shape);
    }
    Ok(())
}
fn seed(bytes: &[u8]) -> Result<(), DiagnosticWireError> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(DiagnosticWireError::Capacity);
    }
    std::str::from_utf8(bytes).map(|_| ()).map_err(|_| DiagnosticWireError::Integrity)
}
fn selection(choice: DiagnosticSelection<'_>) -> Result<(), DiagnosticWireError> {
    if choice.destination.is_empty()
        || choice.destination.len() > 2048
        || !choice.destination.len().is_multiple_of(2)
    {
        return Err(DiagnosticWireError::Capacity);
    }
    if choice.destination.chunks_exact(2).any(|c| c == [0, 0]) {
        return Err(DiagnosticWireError::Shape);
    }
    Ok(())
}
pub(super) const fn bound(kind: DiagnosticTransferKind) -> u64 {
    match kind {
        DiagnosticTransferKind::Summary => 65_536,
        DiagnosticTransferKind::View => 262_144,
        DiagnosticTransferKind::Attachment(AttachmentKind::Minidump) => 268_435_456,
        DiagnosticTransferKind::Attachment(_) => 262_144,
    }
}
pub(super) fn check_chunk(chunk: DiagnosticChunk<'_>) -> Result<(), DiagnosticWireError> {
    if chunk.bytes.is_empty()
        || chunk.bytes.len() > MAX_CHUNK_BYTES
        || chunk.total == 0
        || chunk.total > bound(chunk.kind)
    {
        return Err(DiagnosticWireError::Capacity);
    }
    if chunk.offset.checked_add(chunk.bytes.len() as u64).is_none_or(|end| end > chunk.total) {
        return Err(DiagnosticWireError::Chunk);
    }
    Ok(())
}
