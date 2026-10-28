// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate bounded frame storage incrementally and retain its reservation immutably.

use crate::execution::{Generation, MemoryClaim, MemoryClaimError, MemoryLease, SurfaceId};
use std::{error::Error, fmt, num::NonZeroUsize, sync::Arc};

/// Describe the byte order and alpha representation of prepared pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameFormat {
    /// Top-row-first red, green, blue, alpha bytes; each color channel is at most alpha.
    PremultipliedRgba8,
}

/// Bound tightly packed device-pixel dimensions and logical-to-device scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameShape {
    width: u32,
    height: u32,
    scale: f64,
    stride: usize,
    byte_len: usize,
}

impl FrameShape {
    /// Validate nonzero dimensions at most 4096 and finite `0 < scale <= 4`.
    ///
    /// Width/height are device pixels; scale maps logical coordinates to device
    /// pixels. Storage has exactly four bytes per pixel without row padding.
    ///
    /// # Errors
    /// Rejects invalid dimensions/scale or overflowing stride/storage arithmetic.
    pub fn new(width: u32, height: u32, scale: f64) -> Result<Self, FrameError> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(FrameError::InvalidDimensions);
        }
        if !scale.is_finite() || scale <= 0.0 || scale > 4.0 {
            return Err(FrameError::InvalidScale);
        }
        let stride = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or(FrameError::SizeOverflow)?;
        let byte_len = usize::try_from(height)
            .ok()
            .and_then(|height| stride.checked_mul(height))
            .ok_or(FrameError::SizeOverflow)?;
        Ok(Self { width, height, scale, stride, byte_len })
    }

    /// Return the device-pixel width.
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Return the device-pixel height.
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Return the finite logical-to-device scale.
    pub const fn scale(self) -> f64 {
        self.scale
    }
    /// Return the tightly packed row stride in bytes.
    pub const fn stride(self) -> usize {
        self.stride
    }
    /// Return the exact required pixel-storage length in bytes.
    pub const fn byte_len(self) -> usize {
        self.byte_len
    }
    /// Return the explicit top-row-first premultiplied byte format.
    pub const fn format(self) -> FrameFormat {
        FrameFormat::PremultipliedRgba8
    }
}

/// Separate a surface state revision from its increasing animation samples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameIdentity {
    /// Application-owned surface receiving the prepared result.
    pub surface: SurfaceId,
    /// State revision captured before preparation, including shape and visibility.
    pub state_generation: Generation,
    /// Increasing prepared sample within that state; newer requests do not invalidate it.
    pub sequence: Generation,
}

/// Share immutable validated pixels and exactly one retained allocation/byte charge.
///
/// Clones share storage. The vector and its lease remain private, so borrowed pixels
/// cannot outlive the prepared owner and callers cannot extract unguarded storage.
#[derive(Clone)]
#[must_use = "Retain the prepared frame while its immutable pixel storage is in use"]
pub struct PreparedFrame {
    shape: FrameShape,
    identity: FrameIdentity,
    storage: Arc<FrameStorage>,
}

impl PreparedFrame {
    /// Return validated shape metadata without scanning pixels.
    pub const fn shape(&self) -> FrameShape {
        self.shape
    }
    /// Return captured surface/state/sample identity without changing its ownership.
    pub const fn identity(&self) -> FrameIdentity {
        self.identity
    }
    /// Borrow immutable top-row-first premultiplied RGBA8 bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.storage.pixels
    }
    /// Return the device-pixel width.
    pub const fn width(&self) -> u32 {
        self.shape.width()
    }
    /// Return the device-pixel height.
    pub const fn height(&self) -> u32 {
        self.shape.height()
    }
    /// Return the tightly packed byte stride.
    pub const fn stride(&self) -> usize {
        self.shape.stride()
    }
    /// Return the finite logical-to-device scale.
    pub const fn scale(&self) -> f64 {
        self.shape.scale()
    }
    /// Return the explicit pixel format.
    pub const fn format(&self) -> FrameFormat {
        self.shape.format()
    }
    /// Return the shared admitted byte reservation without exposing its release port.
    pub fn reserved_bytes(&self) -> u64 {
        self.storage.claim.reserved_bytes()
    }

    /// Claim distinct presenter storage from this frame's original admitted reservation.
    ///
    /// Claim before allocating a native copy and retain the non-cloneable guard
    /// until that allocation is released or [`Self::fund_storage`] transfers its credit.
    /// The guard may outlive every source-frame
    /// clone, preserving admission accounting across resize and native teardown.
    /// This checks aggregate capacity without allocating or exposing scheduling ports.
    ///
    /// # Errors
    /// Rejects storage exceeding unclaimed bytes in the already-admitted reservation.
    pub fn claim_storage(&self, bytes: u64) -> Result<MemoryClaim, MemoryClaimError> {
        self.storage.claim.claim_sibling(bytes)
    }

    /// Fund retained presenter storage from this accepted frame without reallocating its pixels.
    ///
    /// The allocation stays charged throughout transfer: replacement credit is claimed before
    /// releasing the previous claim. Repeated exposure of the same reservation is a no-op.
    /// The caller retains the allocation and its unchanged byte extent and thread affinity.
    /// # Errors
    /// Insufficient replacement capacity preserves the original claim and allocation.
    pub fn fund_storage(&self, allocation: &mut MemoryClaim) -> Result<(), MemoryClaimError> {
        self.storage.claim.fund_sibling(allocation)
    }
}

impl fmt::Debug for PreparedFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedFrame")
            .field("shape", &self.shape)
            .field("identity", &self.identity)
            .field("byte_len", &self.storage.pixels.len())
            .finish_non_exhaustive()
    }
}

/// Keep one pixel allocation attached to its admitted reservation until last drop.
struct FrameStorage {
    pixels: Vec<u8>,
    // Rust drops fields in declaration order: free pixels before reusing the claim.
    claim: MemoryClaim,
}

/// Own unpublished pixels while bounded CPU steps validate every alpha relationship.
///
/// Construction checks shape/storage/charge in constant time. Validation examines
/// at most 65536 pixels per call. The charge covers actual vector capacity, not
/// merely length; Arc bookkeeping, renderer scratch and native allocations require
/// separate adapter accounting and are not claimed to be covered by this check.
/// Each independent pixel allocation claims its capacity from the shared lease;
/// cloning a lease cannot admit storage beyond its aggregate reservation.
pub struct FrameValidator {
    shape: FrameShape,
    identity: FrameIdentity,
    pixels: Vec<u8>,
    claim: MemoryClaim,
    validated: usize,
    failure: Option<FrameError>,
}

impl FrameValidator {
    /// Own tightly packed pixels and their already-admitted memory reservation.
    ///
    /// # Errors
    /// Rejects a mismatched byte length, a charge smaller than vector capacity or
    /// insufficient unclaimed capacity shared with other retained allocations.
    pub fn new(
        shape: FrameShape,
        identity: FrameIdentity,
        pixels: Vec<u8>,
        lease: MemoryLease,
    ) -> Result<Self, FrameError> {
        if pixels.len() != shape.byte_len() {
            return Err(FrameError::LengthMismatch {
                expected: shape.byte_len(),
                actual: pixels.len(),
            });
        }
        let required_bytes =
            u64::try_from(pixels.capacity()).map_err(|_| FrameError::SizeOverflow)?;
        if lease.bytes() < required_bytes {
            return Err(FrameError::InsufficientReservation {
                required_bytes,
                reserved_bytes: lease.bytes(),
            });
        }
        let claim = lease.claim(required_bytes).map_err(FrameError::AllocationBudget)?;
        Ok(Self { shape, identity, pixels, claim, validated: 0, failure: None })
    }

    /// Validate up to `min(max, 65536)` further pixels and report full completion.
    ///
    /// # Errors
    /// Rejects any color channel greater than alpha. An invalid validator remains
    /// failed on every subsequent call and cannot produce a prepared frame.
    pub fn validate_next(&mut self, max: NonZeroUsize) -> Result<bool, FrameError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let total = self.shape.byte_len() / 4;
        let end = self.validated + (total - self.validated).min(max.get().min(65536));
        for index in self.validated..end {
            let offset = index * 4;
            let pixel = &self.pixels[offset..offset + 4];
            if pixel[..3].iter().any(|channel| *channel > pixel[3]) {
                let error = FrameError::InvalidPremultiplication { pixel: index };
                self.failure = Some(error);
                return Err(error);
            }
        }
        self.validated = end;
        Ok(end == total)
    }

    /// Transfer fully validated storage in constant time without copying or rescanning.
    ///
    /// # Errors
    /// Rejects an earlier alpha failure or incomplete validation.
    pub fn finish(self) -> Result<PreparedFrame, FrameError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let total = self.shape.byte_len() / 4;
        if self.validated != total {
            return Err(FrameError::ValidationIncomplete {
                validated_pixels: self.validated,
                total_pixels: total,
            });
        }
        Ok(PreparedFrame {
            shape: self.shape,
            identity: self.identity,
            storage: Arc::new(FrameStorage { pixels: self.pixels, claim: self.claim }),
        })
    }
}

/// Reject invalid shape, storage, reservation or incremental alpha validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    /// Dimensions are zero or exceed the 4096-device-pixel limit.
    InvalidDimensions,
    /// Scale is nonfinite, nonpositive or exceeds four.
    InvalidScale,
    /// Checked byte-stride, storage-length or reservation arithmetic overflowed.
    SizeOverflow,
    /// Supplied tightly packed byte length differs from validated shape.
    LengthMismatch {
        /// Shape-required byte length.
        expected: usize,
        /// Actual supplied byte length.
        actual: usize,
    },
    /// Existing admission does not cover the actual pixel allocation capacity.
    InsufficientReservation {
        /// Actual retained vector capacity in bytes.
        required_bytes: u64,
        /// Existing shared admission charge in bytes.
        reserved_bytes: u64,
    },
    /// Other retained allocations leave insufficient capacity within this charge.
    AllocationBudget(MemoryClaimError),
    /// A pixel has a color channel greater than alpha.
    InvalidPremultiplication {
        /// Top-row-first zero-based pixel index.
        pixel: usize,
    },
    /// Finishing was attempted before every pixel was validated.
    ValidationIncomplete {
        /// Number of fully validated pixels.
        validated_pixels: usize,
        /// Number of pixels requiring validation.
        total_pixels: usize,
    },
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => {
                formatter.write_str("Frame dimensions must be between 1 and 4096")
            }
            Self::InvalidScale => {
                formatter.write_str("Frame scale must be finite and greater than zero up to four")
            }
            Self::SizeOverflow => formatter.write_str("Frame storage arithmetic overflowed"),
            Self::LengthMismatch { expected, actual } => {
                write!(formatter, "Frame requires {expected} bytes, received {actual}")
            }
            Self::InsufficientReservation { required_bytes, reserved_bytes } => write!(
                formatter,
                "Frame allocation requires {required_bytes} admitted bytes, received {reserved_bytes}"
            ),
            Self::AllocationBudget(cause) => fmt::Display::fmt(cause, formatter),
            Self::InvalidPremultiplication { pixel } => {
                write!(formatter, "Frame pixel {pixel} is not premultiplied")
            }
            Self::ValidationIncomplete { validated_pixels, total_pixels } => {
                write!(formatter, "Frame validated {validated_pixels} of {total_pixels} pixels")
            }
        }
    }
}
impl Error for FrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AllocationBudget(cause) => Some(cause),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
