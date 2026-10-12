// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate complete exact streaming transfers without allocating or activating a view.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::incident::{
    AttachmentKind, DiagnosticKey, DiagnosticRole, ReportSessionId,
};
/// Exact current stream scope, distinct from its potentially historical incident.
/// These checked copied facts retain correlation without granting native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticTransferBinding {
    key: DiagnosticKey,
    context: DiagnosticContext,
    session: Option<ReportSessionId>,
}
impl DiagnosticTransferBinding {
    /// Derive correlation only from a strictly checked stream command.
    /// Request IDs change per chunk; the original full stream context does not.
    /// # Errors
    /// Nonstream commands or invalid role/context shapes reject.
    pub fn from_frame(frame: &DiagnosticFrame<'_>) -> Result<Self, DiagnosticWireError> {
        validation::check(frame)?;
        let session = match frame.command {
            DiagnosticCommand::BeginView { session, .. }
            | DiagnosticCommand::ReplaceView { session, .. }
            | DiagnosticCommand::ViewChunk { session, .. }
            | DiagnosticCommand::EndView { session, .. } => Some(session),
            DiagnosticCommand::Publish(_)
            | DiagnosticCommand::RetainExport(_)
            | DiagnosticCommand::ExportChunk(_)
            | DiagnosticCommand::FinishExport(_) => None,
            _ => return Err(DiagnosticWireError::Shape),
        };
        Ok(Self { key: frame.key, context: frame.context, session })
    }
    /// Observe complete current native-session matching facts.
    pub const fn key(self) -> DiagnosticKey {
        self.key
    }
    /// Observe exact current operation/deadline/UTC and historical incident/revision.
    pub const fn context(self) -> DiagnosticContext {
        self.context
    }
    /// Observe a reporter session only for actual reporter stream commands.
    pub const fn session(self) -> Option<ReportSessionId> {
        self.session
    }
}
#[derive(Clone, Default)]
struct Utf8 {
    pending: [u8; 4],
    held: usize,
    needed: usize,
}
impl Utf8 {
    fn push(&mut self, bytes: &[u8]) -> Result<(), DiagnosticWireError> {
        for &byte in bytes {
            if self.held == 0 {
                if byte < 128 {
                    continue;
                }
                self.needed = match byte {
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => return Err(DiagnosticWireError::Integrity),
                };
            } else if !(0x80..=0xbf).contains(&byte) {
                return Err(DiagnosticWireError::Integrity);
            }
            self.pending[self.held] = byte;
            self.held += 1;
            if self.held == self.needed {
                std::str::from_utf8(&self.pending[..self.held])
                    .map_err(|_| DiagnosticWireError::Integrity)?;
                self.held = 0;
            }
        }
        Ok(())
    }
}
/// Complete transfer data receipt; grants no buffer, filesystem or native authority.
/// A presentation consumer still needs its actual charged complete allocation.
pub struct CompletedDiagnosticTransfer {
    binding: DiagnosticTransferBinding,
    kind: DiagnosticTransferKind,
    total: u64,
    sha256: [u8; 32],
}
impl CompletedDiagnosticTransfer {
    /// Original exact current key/context and optional reporter session.
    pub const fn binding(&self) -> DiagnosticTransferBinding {
        self.binding
    }
    /// Original exact stream kind.
    pub const fn kind(&self) -> DiagnosticTransferKind {
        self.kind
    }
    /// Checked complete byte count.
    pub const fn bytes(&self) -> u64 {
        self.total
    }
    /// Checked complete digest.
    pub const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}
/// Fixed streaming validation state; actual byte custody remains with its caller.
pub struct DiagnosticTransfer {
    binding: DiagnosticTransferBinding,
    kind: DiagnosticTransferKind,
    total: u64,
    sha256: [u8; 32],
    offset: u64,
    hasher: Sha256,
    utf8: Utf8,
    finished: bool,
}
impl DiagnosticTransfer {
    /// Retain exact bounded stream metadata without allocating byte storage.
    /// # Errors
    /// Zero/excess totals reject before any state admission.
    pub fn new(
        binding: DiagnosticTransferBinding,
        kind: DiagnosticTransferKind,
        total: u64,
        sha256: [u8; 32],
    ) -> Result<Self, DiagnosticWireError> {
        if (kind == DiagnosticTransferKind::View)
            != (binding.key.role() == DiagnosticRole::Reporter)
        {
            return Err(DiagnosticWireError::Role);
        }
        if total == 0 || total > validation::bound(kind) {
            return Err(DiagnosticWireError::Capacity);
        }
        Ok(Self {
            binding,
            kind,
            total,
            sha256,
            offset: 0,
            hasher: Sha256::new(),
            utf8: Utf8::default(),
            finished: false,
        })
    }
    /// Validate contiguous chunks and split UTF8 before advancing digest/offset.
    /// # Errors
    /// Wrong binding/kind/digest/offset/total or invalid UTF8 preserves prior state.
    pub fn accept(
        &mut self,
        binding: DiagnosticTransferBinding,
        chunk: DiagnosticChunk<'_>,
    ) -> Result<(), DiagnosticWireError> {
        if self.finished {
            return Err(DiagnosticWireError::Stale);
        }
        validation::check_chunk(chunk)?;
        if binding != self.binding
            || chunk.kind != self.kind
            || chunk.total != self.total
            || chunk.sha256 != self.sha256
            || chunk.offset != self.offset
        {
            return Err(DiagnosticWireError::Chunk);
        }
        let text =
            !matches!(self.kind, DiagnosticTransferKind::Attachment(AttachmentKind::Minidump));
        let mut utf8 = self.utf8.clone();
        if text {
            utf8.push(chunk.bytes)?
        }
        self.hasher.update(chunk.bytes);
        self.utf8 = utf8;
        self.offset += chunk.bytes.len() as u64;
        Ok(())
    }
    /// Produce completion once only after exact full digest and whole UTF8 validate.
    /// # Errors
    /// Partial, changed/stale binding, digest mismatch or truncated UTF8 cannot complete.
    pub fn finish(
        &mut self,
        binding: DiagnosticTransferBinding,
        sha256: [u8; 32],
    ) -> Result<CompletedDiagnosticTransfer, DiagnosticWireError> {
        if self.finished {
            return Err(DiagnosticWireError::Stale);
        }
        if binding != self.binding || sha256 != self.sha256 {
            return Err(DiagnosticWireError::Chunk);
        }
        if self.offset != self.total || self.utf8.held != 0 {
            return Err(DiagnosticWireError::Incomplete);
        }
        let actual: [u8; 32] = self.hasher.clone().finalize().into();
        if actual != self.sha256 {
            return Err(DiagnosticWireError::Integrity);
        }
        self.finished = true;
        Ok(CompletedDiagnosticTransfer {
            binding: self.binding,
            kind: self.kind,
            total: self.total,
            sha256: self.sha256,
        })
    }
    /// Inspect checked progress, never presentation readiness.
    pub const fn accepted_bytes(&self) -> u64 {
        self.offset
    }
}
