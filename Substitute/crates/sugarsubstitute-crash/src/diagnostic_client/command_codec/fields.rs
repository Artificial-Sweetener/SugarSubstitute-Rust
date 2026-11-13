// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own closed field discriminators and bounded borrowed chunk/selection encoding.
use super::super::{reader::Reader, writer::Writer, *};
use DiagnosticWireError as E;
use sugarsubstitute_contracts::incident::*;
pub(super) fn session(r: &mut Reader<'_>) -> Result<ReportSessionId, E> {
    ReportSessionId::new(r.u64()?).map_err(|_| E::Identity)
}
pub(super) fn revision(r: &mut Reader<'_>) -> Result<IncidentRevision, E> {
    IncidentRevision::new(r.u64()?).map_err(|_| E::Identity)
}
pub(super) fn failure(code: u8) -> Result<ReportFailure, E> {
    Ok(match code {
        1 => ReportFailure::Launch,
        2 => ReportFailure::Load,
        3 => ReportFailure::Presentation,
        4 => ReportFailure::UnexpectedExit,
        5 => ReportFailure::Acknowledgement,
        6 => ReportFailure::Export,
        7 => ReportFailure::Retirement,
        _ => return Err(E::Schema),
    })
}
pub(super) fn failure_code(value: ReportFailure) -> u8 {
    match value {
        ReportFailure::Launch => 1,
        ReportFailure::Load => 2,
        ReportFailure::Presentation => 3,
        ReportFailure::UnexpectedExit => 4,
        ReportFailure::Acknowledgement => 5,
        ReportFailure::Export => 6,
        ReportFailure::Retirement => 7,
    }
}
pub(super) fn action(code: u8) -> Result<ReportAction, E> {
    Ok(match code {
        0 => ReportAction::CopyText,
        1 => ReportAction::Export(ExportSelection::TextOnly),
        2 => ReportAction::Export(ExportSelection::TextAndNativeDump),
        3 => ReportAction::Dismiss,
        4 => ReportAction::DismissAndRequestRestart,
        _ => return Err(E::Schema),
    })
}
pub(super) fn action_code(a: ReportAction) -> u8 {
    match a {
        ReportAction::CopyText => 0,
        ReportAction::Export(ExportSelection::TextOnly) => 1,
        ReportAction::Export(ExportSelection::TextAndNativeDump) => 2,
        ReportAction::Dismiss => 3,
        ReportAction::DismissAndRequestRestart => 4,
    }
}
pub(super) fn kind_code(k: DiagnosticTransferKind) -> u8 {
    match k {
        DiagnosticTransferKind::Summary => 0,
        DiagnosticTransferKind::View => 1,
        DiagnosticTransferKind::Attachment(a) => match a {
            AttachmentKind::Startup => 2,
            AttachmentKind::Fault => 3,
            AttachmentKind::Runtime => 4,
            AttachmentKind::Log => 5,
            AttachmentKind::SymbolizedFault => 6,
            AttachmentKind::Minidump => 7,
        },
    }
}
pub(super) fn kind(code: u8) -> Result<DiagnosticTransferKind, E> {
    Ok(match code {
        0 => DiagnosticTransferKind::Summary,
        1 => DiagnosticTransferKind::View,
        n => DiagnosticTransferKind::Attachment(match n {
            2 => AttachmentKind::Startup,
            3 => AttachmentKind::Fault,
            4 => AttachmentKind::Runtime,
            5 => AttachmentKind::Log,
            6 => AttachmentKind::SymbolizedFault,
            7 => AttachmentKind::Minidump,
            _ => return Err(E::Schema),
        }),
    })
}
pub(super) fn write_chunk(w: &mut Writer<'_>, c: DiagnosticChunk<'_>) -> Result<(), E> {
    w.u8(kind_code(c.kind))?;
    w.u64(c.total)?;
    w.u64(c.offset)?;
    w.put(&c.sha256)
}
pub(super) fn read_chunk<'a>(
    r: &mut Reader<'_>,
    bytes: &'a [u8],
) -> Result<DiagnosticChunk<'a>, E> {
    Ok(DiagnosticChunk {
        kind: kind(r.u8()?)?,
        total: r.u64()?,
        offset: r.u64()?,
        sha256: r.array()?,
        bytes,
    })
}
pub(super) fn write_choice(w: &mut Writer<'_>, s: DiagnosticSelection<'_>) -> Result<(), E> {
    w.u64(s.id.get())?;
    w.u8(match s.selection {
        ExportSelection::TextOnly => 0,
        ExportSelection::TextAndNativeDump => 1,
    })?;
    w.u8(u8::from(s.overwrite))
}
pub(super) fn read_choice<'a>(
    r: &mut Reader<'_>,
    destination: &'a [u8],
) -> Result<DiagnosticSelection<'a>, E> {
    Ok(DiagnosticSelection {
        id: DiagnosticRequestId::new(r.u64()?)?,
        selection: match r.u8()? {
            0 => ExportSelection::TextOnly,
            1 => ExportSelection::TextAndNativeDump,
            _ => return Err(E::Schema),
        },
        overwrite: r.boolean()?,
        destination,
    })
}
pub(super) fn write_result(w: &mut Writer<'_>, r: DiagnosticActionResult) -> Result<(), E> {
    w.u8(match r {
        DiagnosticActionResult::Success => 0,
        DiagnosticActionResult::Failed(f) => failure_code(f),
    })
}
pub(super) fn read_result(r: &mut Reader<'_>) -> Result<DiagnosticActionResult, E> {
    Ok(match r.u8()? {
        0 => DiagnosticActionResult::Success,
        c => DiagnosticActionResult::Failed(failure(c)?),
    })
}
const FAILURES: [E; 13] = [
    E::Capacity,
    E::Schema,
    E::Shape,
    E::Identity,
    E::Time,
    E::Role,
    E::ChangedDuplicate,
    E::RetiredOperation,
    E::Busy,
    E::Stale,
    E::Chunk,
    E::Integrity,
    E::Incomplete,
];
pub(super) fn write_reply(w: &mut Writer<'_>, reply: DiagnosticReply) -> Result<(), E> {
    match reply {
        DiagnosticReply::Failed(failure) => {
            w.u8(2)?;
            w.u8(operation_failure_code(failure))
        }
        DiagnosticReply::Rejected(error) => {
            w.u8(0)?;
            let code = FAILURES.iter().position(|e| *e == error).ok_or(E::Schema)?;
            w.u8(u8::try_from(code).map_err(|_| E::Schema)?)
        }
        DiagnosticReply::Completed { bytes, sha256, revision } => {
            w.u8(1)?;
            w.u64(bytes)?;
            w.put(&sha256)?;
            w.u8(u8::from(revision.is_some()))?;
            if let Some(r) = revision {
                w.u64(r.get())?
            }
            Ok(())
        }
    }
}
pub(super) fn read_reply(r: &mut Reader<'_>) -> Result<DiagnosticReply, E> {
    match r.u8()? {
        2 => Ok(DiagnosticReply::Failed(operation_failure(r.u8()?)?)),
        0 => {
            let code = usize::from(r.u8()?);
            Ok(DiagnosticReply::Rejected(*FAILURES.get(code).ok_or(E::Schema)?))
        }
        1 => {
            let bytes = r.u64()?;
            let sha256 = r.array()?;
            let revision = if r.boolean()? { Some(revision(r)?) } else { None };
            Ok(DiagnosticReply::Completed { bytes, sha256, revision })
        }
        _ => Err(E::Schema),
    }
}

pub(super) fn operation_failure_code(f: DiagnosticOperationFailure) -> u8 {
    use DiagnosticOperationFailure as F;
    match f {
        F::Admission => 0,
        F::Cancelled => 1,
        F::Deadline => 2,
        F::Capacity => 3,
        F::Identity => 4,
        F::Storage => 5,
        F::Recovery => 6,
        F::Capture => 7,
        F::Symbols => 8,
        F::Export => 9,
        F::Presentation => 10,
        F::Retirement => 11,
    }
}
pub(super) fn operation_failure(code: u8) -> Result<DiagnosticOperationFailure, E> {
    use DiagnosticOperationFailure as F;
    Ok(match code {
        0 => F::Admission,
        1 => F::Cancelled,
        2 => F::Deadline,
        3 => F::Capacity,
        4 => F::Identity,
        5 => F::Storage,
        6 => F::Recovery,
        7 => F::Capture,
        8 => F::Symbols,
        9 => F::Export,
        10 => F::Presentation,
        11 => F::Retirement,
        _ => return Err(E::Schema),
    })
}
