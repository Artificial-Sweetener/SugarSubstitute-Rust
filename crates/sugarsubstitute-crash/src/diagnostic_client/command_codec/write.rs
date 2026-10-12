// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode only closed command-specific fields without allocation.
use super::super::{writer::Writer, *};
use super::fields::*;
use DiagnosticWireError as E;
use sugarsubstitute_contracts::incident::*;
pub(in crate::diagnostic_client) fn write<'a>(
    c: DiagnosticCommand<'a>,
    w: &mut Writer<'_>,
) -> Result<(u8, &'a [u8]), E> {
    use DiagnosticCommand as C;
    let empty: &[u8] = &[];
    let result = match c {
        C::Result { kind, request_sha256, reply } => {
            w.u8(kind.code())?;
            w.put(&request_sha256)?;
            write_reply(w, reply)?;
            (31, empty)
        }
        C::OpenProtectedRoot => (1, empty),
        C::Recover => (2, empty),
        C::Seed(bytes) => (3, bytes),
        C::Publish(DiagnosticPublication::Begin { expected, demand, seed }) => {
            match expected {
                DiagnosticExpectedSummary::Absent => w.u8(0)?,
                DiagnosticExpectedSummary::Exact { revision, sha256 } => {
                    w.u8(1)?;
                    w.u64(revision.get())?;
                    w.put(&sha256)?
                }
            }
            w.u64(demand.payload_bytes())?;
            w.u32(demand.payload_groups())?;
            w.u32(demand.summary_slots())?;
            w.u64(demand.staging_headroom())?;
            (4, seed)
        }
        C::Publish(DiagnosticPublication::Chunk(c)) => {
            write_chunk(w, c)?;
            (5, c.bytes)
        }
        C::Publish(DiagnosticPublication::Finish { bytes, sha256 }) => {
            w.u64(bytes)?;
            w.put(&sha256)?;
            (6, empty)
        }
        C::Load => (7, empty),
        C::RetainExport(s) => {
            write_choice(w, s)?;
            (8, s.destination)
        }
        C::ExportChunk(c) => {
            write_chunk(w, c)?;
            (9, c.bytes)
        }
        C::FinishExport(sha256) => {
            w.put(&sha256)?;
            (10, empty)
        }
        C::Acknowledge { session, restart } => {
            w.u64(session.get())?;
            w.u8(u8::from(restart))?;
            (11, empty)
        }
        C::RecordAttempt { session, attempt } => {
            w.u64(session.get())?;
            w.u8(match attempt {
                ReportAttempt::Presented => 0,
                ReportAttempt::Failed(f) => failure_code(f),
            })?;
            (12, empty)
        }
        C::RetainDump(id) => {
            w.put(id.bytes())?;
            (13, empty)
        }
        C::Symbolize { dump, release } => {
            w.put(dump.bytes())?;
            w.put(&release)?;
            (14, empty)
        }
        C::DiscardCapture(id) => {
            w.put(id.bytes())?;
            (15, empty)
        }
        C::Retire => (16, empty),
        C::BeginView { session, total, sha256 } => {
            w.u64(session.get())?;
            w.u64(total)?;
            w.put(&sha256)?;
            (17, empty)
        }
        C::ViewChunk { session, chunk } => {
            w.u64(session.get())?;
            write_chunk(w, chunk)?;
            (18, chunk.bytes)
        }
        C::EndView { session, sha256 } => {
            w.u64(session.get())?;
            w.put(&sha256)?;
            (19, empty)
        }
        C::ReplaceView { session, old, total, sha256 } => {
            w.u64(session.get())?;
            w.u64(old.get())?;
            w.u64(total)?;
            w.put(&sha256)?;
            (20, empty)
        }
        C::Presented(s) => {
            w.u64(s.get())?;
            (21, empty)
        }
        C::ViewReplaced(s) => {
            w.u64(s.get())?;
            (22, empty)
        }
        C::Action { session, action } => {
            w.u64(session.get())?;
            w.u8(action_code(action))?;
            (23, empty)
        }
        C::SaveSelectionReady { session, selection } => {
            w.u64(session.get())?;
            write_choice(w, selection)?;
            (24, selection.destination)
        }
        C::SaveSelectionCancelled(s) => {
            w.u64(s.get())?;
            (25, empty)
        }
        C::NativeRetired(s) => {
            w.u64(s.get())?;
            (26, empty)
        }
        C::ReporterFailed { session, failure } => {
            w.u64(session.get())?;
            w.u8(failure_code(failure))?;
            (27, empty)
        }
        C::CloseAfterDurableAck { session, acknowledgement } => {
            w.u64(session.get())?;
            w.u64(acknowledgement.at().unix_seconds())?;
            (28, empty)
        }
        C::CopyResult { session, result } => {
            w.u64(session.get())?;
            write_result(w, result)?;
            (29, empty)
        }
        C::ExportSelectionResult { session, result } => {
            w.u64(session.get())?;
            write_result(w, result)?;
            (30, empty)
        }
    };
    Ok(result)
}
