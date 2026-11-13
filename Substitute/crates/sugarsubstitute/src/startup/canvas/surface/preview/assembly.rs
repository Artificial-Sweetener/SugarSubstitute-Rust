// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one admitted encoded preview until complete transfer or worker-owned retirement.

use std::{error::Error, time::Duration};
use sugarsubstitute_contracts::{
    canvas_surface::{
        CanvasSurfacePreviewBegin, CanvasSurfacePreviewChunk, MAX_SURFACE_PREVIEW_CHUNK_BYTES,
        SURFACE_PREVIEW_LEASE_MILLISECONDS,
    },
    execution::{MemoryClaim, MemoryLease, Moment},
};
use sugarsubstitute_image_io::ImageSource;
#[cfg(test)]
mod tests;

/// A custody error never drops the admitted original; the application retires it explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Rejection {
    /// Another token or begin revision does not own this transfer.
    Foreign,
    /// The original begin deadline elapsed, independently of later requests.
    Expired,
    /// A matching transfer supplied a gap, replay, excess or noncanonical chunk length.
    Sequence,
    /// End arrived before every originally declared byte.
    Incomplete,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Preview transfer rejected: {self:?}")
    }
}
impl Error for Rejection {}

/// A single contiguous allocation is claimed before reservation and never grows while receiving.
/// Only bounded chunk copies run on the application thread; decoding runs on the image worker.
pub(super) struct Assembly {
    pub(super) begin: CanvasSurfacePreviewBegin,
    pub(super) revision: u64,
    pub(super) deadline: Moment,
    bytes: Option<(Vec<u8>, MemoryClaim)>,
}

impl Assembly {
    /// Admit original capacity and metadata before retaining the first byte or starting a lease.
    /// Failure preserves existing transfers at the caller; it does not grant native authority.
    pub(super) fn new(
        begin: CanvasSurfacePreviewBegin,
        revision: u64,
        resident: &MemoryLease,
        now: Moment,
    ) -> Result<Self, Box<dyn Error>> {
        let deadline =
            now.checked_add(Duration::from_millis(SURFACE_PREVIEW_LEASE_MILLISECONDS))?;
        let length = usize::try_from(begin.byte_length())?;
        let claim = resident.claim(
            begin.byte_length().checked_add(32 * 1024 + 128).ok_or("Preview admission overflow")?,
        )?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        if bytes.capacity() > length {
            return Err("Preview allocation exceeds admitted capacity".into());
        }
        Ok(Self { begin, revision, deadline, bytes: Some((bytes, claim)) })
    }

    /// Match immutable lease identity separately from the changing surface operation revision.
    pub(super) fn owns(&self, token: &str, revision: u64) -> bool {
        self.begin.identity().token() == token && self.revision == revision
    }

    /// Copy one already validated canonical chunk without allocating or renewing the deadline.
    pub(super) fn receive(
        &mut self,
        chunk: &CanvasSurfacePreviewChunk,
        now: Moment,
    ) -> Result<(), Rejection> {
        if !self.owns(chunk.token(), chunk.lease_revision()) {
            return Err(Rejection::Foreign);
        }
        if now >= self.deadline {
            return Err(Rejection::Expired);
        }
        let (bytes, _) = self.bytes.as_mut().ok_or(Rejection::Sequence)?;
        let remaining = self.begin.byte_length().saturating_sub(bytes.len() as u64);
        let expected = remaining.min(MAX_SURFACE_PREVIEW_CHUNK_BYTES as u64) as usize;
        if chunk.offset() != bytes.len() as u64 || expected == 0 || chunk.data().len() != expected {
            return Err(Rejection::Sequence);
        }
        bytes.extend_from_slice(chunk.data());
        Ok(())
    }

    /// Validate terminal custody independently of native decode or presentation acceptance.
    pub(super) fn complete(
        &self,
        token: &str,
        revision: u64,
        now: Moment,
    ) -> Result<(), Rejection> {
        if !self.owns(token, revision) {
            return Err(Rejection::Foreign);
        }
        if now >= self.deadline {
            return Err(Rejection::Expired);
        }
        if self.bytes.as_ref().map(|(bytes, _)| bytes.len() as u64)
            != Some(self.begin.byte_length())
        {
            return Err(Rejection::Incomplete);
        }
        Ok(())
    }

    /// Transfer a JPEG original without copying; failure keeps complete byte custody here.
    pub(super) fn original(
        &mut self,
        resident: &MemoryLease,
    ) -> Result<ImageSource, Box<dyn Error>> {
        if !self.bytes.as_ref().is_some_and(|(bytes, _)| bytes.starts_with(&[0xff, 0xd8])) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Declared JPEG preview does not contain a JPEG signature",
            )
            .into());
        }
        Ok(ImageSource::memory(&mut self.bytes, resident)?)
    }

    /// Borrow aborted or expired storage for the existing session's storage-worker retirement.
    pub(super) fn retirement(&mut self) -> &mut Option<(Vec<u8>, MemoryClaim)> {
        &mut self.bytes
    }
}
