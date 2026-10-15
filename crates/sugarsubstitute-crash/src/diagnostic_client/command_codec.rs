// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed command tag contract and cohesive reader/writer field modules.
mod fields;
mod read;
mod write;
use super::*;
pub(super) use read::read;
pub(super) use write::write;
pub(super) const fn code(c: DiagnosticCommand<'_>) -> u8 {
    use DiagnosticCommand as C;
    match c {
        C::OpenProtectedRoot => 1,
        C::Recover => 2,
        C::Seed(_) => 3,
        C::Publish(DiagnosticPublication::Begin { .. }) => 4,
        C::Publish(DiagnosticPublication::Chunk(_)) => 5,
        C::Publish(DiagnosticPublication::Finish { .. }) => 6,
        C::Load => 7,
        C::RetainExport(_) => 8,
        C::ExportChunk(_) => 9,
        C::FinishExport(_) => 10,
        C::Acknowledge { .. } => 11,
        C::RecordAttempt { .. } => 12,
        C::RetainDump(_) => 13,
        C::Symbolize { .. } => 14,
        C::DiscardCapture(_) => 15,
        C::Retire => 16,
        C::BeginView { .. } => 17,
        C::ViewChunk { .. } => 18,
        C::EndView { .. } => 19,
        C::ReplaceView { .. } => 20,
        C::Presented(_) => 21,
        C::ViewReplaced(_) => 22,
        C::Action { .. } => 23,
        C::SaveSelectionReady { .. } => 24,
        C::SaveSelectionCancelled(_) => 25,
        C::NativeRetired(_) => 26,
        C::ReporterFailed { .. } => 27,
        C::CloseAfterDurableAck { .. } => 28,
        C::CopyResult { .. } => 29,
        C::ExportSelectionResult { .. } => 30,
        C::Result { .. } => 31,
        C::RestartUnavailable { .. } => 32,
    }
}
