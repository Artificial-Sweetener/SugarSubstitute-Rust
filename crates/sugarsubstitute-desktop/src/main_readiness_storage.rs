// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Claim fixed heap tables before retaining bounded native frame/context observations.

use crate::{
    DesktopError,
    main_readiness_types::{DocumentId, ReadinessFault},
};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{MemoryClaim, MemoryLease},
};

#[derive(Clone, Copy)]
pub(crate) struct FrameRecord {
    pub(crate) id: DocumentId,
    pub(crate) loader: DocumentId,
    pub(crate) root: bool,
    pub(crate) origin_matches: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct ContextRecord {
    pub(crate) id: i64,
    pub(crate) frame: DocumentId,
    pub(crate) default: bool,
    pub(crate) origin: Option<LoopbackOrigin>,
}

/// Release both tables before returning their shared admitted capacity.
pub(crate) struct DocumentTables {
    pub(crate) frames: Vec<FrameRecord>,
    pub(crate) contexts: Vec<ContextRecord>,
    _frames_claim: MemoryClaim,
    _contexts_claim: MemoryClaim,
}
impl DocumentTables {
    pub(crate) const FRAMES: usize = 32;
    pub(crate) const CONTEXTS: usize = 64;

    pub(crate) fn new(lease: &MemoryLease) -> Result<Self, DesktopError> {
        let frame_bytes = Self::FRAMES.checked_mul(size_of::<FrameRecord>()).ok_or_else(|| {
            DesktopError::message("claim_readiness_tables", "Frame capacity overflow")
        })?;
        let context_bytes =
            Self::CONTEXTS.checked_mul(size_of::<ContextRecord>()).ok_or_else(|| {
                DesktopError::message("claim_readiness_tables", "Context capacity overflow")
            })?;
        let frames_claim = lease
            .claim(frame_bytes as u64)
            .map_err(|e| DesktopError::new("claim_readiness_frames", e))?;
        let contexts_claim = lease
            .claim(context_bytes as u64)
            .map_err(|e| DesktopError::new("claim_readiness_contexts", e))?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(Self::FRAMES)
            .map_err(|e| DesktopError::new("allocate_readiness_frames", e))?;
        let mut contexts = Vec::new();
        contexts
            .try_reserve_exact(Self::CONTEXTS)
            .map_err(|e| DesktopError::new("allocate_readiness_contexts", e))?;
        if frames.capacity() != Self::FRAMES || contexts.capacity() != Self::CONTEXTS {
            return Err(DesktopError::message(
                "allocate_readiness_tables",
                "Allocator capacity exceeds exact claims",
            ));
        }
        Ok(Self { frames, contexts, _frames_claim: frames_claim, _contexts_claim: contexts_claim })
    }

    pub(crate) fn frame(&self, id: DocumentId) -> Option<FrameRecord> {
        self.frames.iter().find(|record| record.id == id).copied()
    }
    pub(crate) fn context(&self, id: i64) -> Option<ContextRecord> {
        self.contexts.iter().find(|record| record.id == id).copied()
    }
    pub(crate) fn insert_frame(&mut self, record: FrameRecord) -> Result<(), ReadinessFault> {
        if let Some(existing) = self.frames.iter_mut().find(|f| f.id == record.id) {
            *existing = record;
        } else {
            if self.frames.len() == Self::FRAMES {
                return Err(ReadinessFault::ObservationOverflow);
            }
            self.frames.push(record);
        }
        Ok(())
    }
    pub(crate) fn insert_context(&mut self, record: ContextRecord) -> Result<(), ReadinessFault> {
        if self.context(record.id).is_some() {
            return Err(ReadinessFault::MalformedObservation);
        }
        if self.contexts.len() == Self::CONTEXTS {
            return Err(ReadinessFault::ObservationOverflow);
        }
        self.contexts.push(record);
        Ok(())
    }
}
