// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain admitted image storage through incremental initialization and publication transfer.
use super::*;

/// Owned storage precedes its charge in destruction order.
pub(super) struct Image {
    bytes: Vec<u8>,
    pixels: Option<Bitmap>,
    shape: FrameShape,
    position: usize,
    claim: MemoryClaim,
}
impl Image {
    /// Claim capacity before allocation; large zero-initialization occurs in cooperative slices.
    pub(super) fn new(shape: FrameShape, lease: &MemoryLease) -> io::Result<Self> {
        let claim = lease.claim(shape.byte_len() as u64).map_err(io::Error::other)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(shape.byte_len()).map_err(io::Error::other)?;
        if bytes.capacity() > shape.byte_len() {
            return Err(invalid());
        }
        Ok(Self { bytes, pixels: None, shape, position: 0, claim })
    }
    /// Reuse the same layer allocation for its next admitted scene.
    pub(super) fn begin_clear(&mut self) {
        self.position = 0;
    }
    /// Clear at most one cooperative slice, including the initial vector initialization.
    pub(super) fn clear(&mut self) -> io::Result<bool> {
        let end = (self.position + PIXELS_PER_STEP * 4).min(self.shape.byte_len());
        if let Some(pixels) = &mut self.pixels {
            pixels.data_mut()[self.position..end].fill(0);
        } else {
            self.bytes.resize(end, 0);
            if end == self.shape.byte_len() {
                let size = tiny_skia::IntSize::from_wh(self.shape.width(), self.shape.height())
                    .ok_or_else(invalid)?;
                self.pixels = Some(
                    Bitmap::from_vec(std::mem::take(&mut self.bytes), size).ok_or_else(invalid)?,
                );
            }
        }
        self.position = end;
        Ok(end == self.shape.byte_len())
    }
    /// Borrow only initialized pixels without exposing storage beyond this custody.
    pub(super) fn pixels_mut(&mut self) -> io::Result<&mut Bitmap> {
        if self.position != self.shape.byte_len() {
            return Err(invalid());
        }
        self.pixels.as_mut().ok_or_else(invalid)
    }
    /// Keep the original charge live until the validator adopts the same allocation.
    pub(super) fn finish(self) -> io::Result<(Vec<u8>, MemoryClaim)> {
        if self.position != self.shape.byte_len() {
            return Err(invalid());
        }
        Ok((self.pixels.ok_or_else(invalid)?.take(), self.claim))
    }
}
