// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Version the reconstructed source's packing and color interpretation independently of paths.
use crate::{ChunkLayout, ImportError, Packing};
use sha2::{Digest, Sha256};
use sugarsubstitute_image_ops::{AssetId, Extent};

const MAGIC: &[u8; 8] = b"SSRAST01";
/// Encoded source transfer in sRGB primaries. Alpha always remains linear coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transfer {
    /// IEC sRGB's piecewise transfer, also the convention for untagged generation outputs.
    Srgb,
    /// PNG source gamma: encoded samples are raised to the reciprocal on decode.
    Gamma(f32),
}
impl Transfer {
    pub(crate) fn validate(self) -> Result<(), ImportError> {
        if let Self::Gamma(gamma) = self
            && (!gamma.is_finite() || gamma <= 0.0)
        {
            return Err(ImportError::Invalid("Unsupported source transfer exponent"));
        }
        Ok(())
    }
    pub(crate) fn linear(self, encoded: f64) -> f32 {
        match self {
            Self::Srgb => sugarsubstitute_image_ops::srgb_to_linear(encoded),
            Self::Gamma(gamma) => encoded.powf(1.0 / f64::from(gamma)) as f32,
        }
    }
}

/// Validated source/archive semantics. Original encoded bytes remain authoritative outside cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterDescriptor {
    layout: ChunkLayout,
    transfer: Transfer,
    content: [u8; 32],
}
impl RasterDescriptor {
    /// Bind exact original-byte SHA-256 to validated decoding and packing semantics.
    /// # Errors
    /// Rejects unsupported transfer exponents and source/archive limits.
    pub fn new(
        extent: Extent,
        packing: Packing,
        transfer: Transfer,
        content: [u8; 32],
    ) -> Result<Self, ImportError> {
        transfer.validate()?;
        Ok(Self { layout: ChunkLayout::new(extent, packing)?, transfer, content })
    }
    /// Region-addressable packed source layout.
    pub const fn layout(self) -> ChunkLayout {
        self.layout
    }
    /// Transfer into canonical linear sRGB primaries.
    pub const fn transfer(self) -> Transfer {
        self.transfer
    }
    /// SHA-256 of the immutable original encoded file, never a pathname or timestamp.
    pub const fn original_hash(self) -> [u8; 32] {
        self.content
    }
    /// Asset identity includes the source interpretation and format epoch as well as original bytes.
    pub fn asset(self) -> AssetId {
        AssetId(Sha256::digest(self.header()).into())
    }
    pub(crate) fn header(self) -> [u8; 128] {
        let mut output = [0; 128];
        output[..8].copy_from_slice(MAGIC);
        output[8..12].copy_from_slice(&self.layout.extent().width().to_le_bytes());
        output[12..16].copy_from_slice(&self.layout.extent().height().to_le_bytes());
        output[16..20].copy_from_slice(&self.layout.rows().to_le_bytes());
        output[20..24].copy_from_slice(&self.layout.packing().bytes().to_le_bytes());
        let (kind, gamma) = match self.transfer {
            Transfer::Srgb => (0_u32, 0_f32),
            Transfer::Gamma(gamma) => (1, gamma),
        };
        output[24..28].copy_from_slice(&kind.to_le_bytes());
        output[28..32].copy_from_slice(&gamma.to_le_bytes());
        output[32..64].copy_from_slice(&self.content);
        output[64..72].copy_from_slice(&self.layout.length().to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(&output[..96]).into();
        output[96..].copy_from_slice(&hash);
        output
    }
    pub(crate) fn from_header(bytes: &[u8; 128]) -> Result<Self, ImportError> {
        let value = |offset| u32::from_le_bytes(std::array::from_fn(|index| bytes[offset + index]));
        if &bytes[..8] != MAGIC || bytes[72..96].iter().any(|byte| *byte != 0) {
            return Err(ImportError::Invalid("Unknown reconstructed image format"));
        }
        let packing = match value(20) {
            4 => Packing::Rgba8,
            8 => Packing::Rgba16,
            _ => return Err(ImportError::Invalid("Invalid reconstructed sample packing")),
        };
        let transfer = match value(24) {
            0 if value(28) == 0 => Transfer::Srgb,
            1 => Transfer::Gamma(f32::from_bits(value(28))),
            _ => return Err(ImportError::Invalid("Invalid reconstructed source transfer")),
        };
        let mut content = [0; 32];
        content.copy_from_slice(&bytes[32..64]);
        let descriptor = Self::new(Extent::new(value(8), value(12))?, packing, transfer, content)?;
        if descriptor.header() != *bytes {
            return Err(ImportError::Invalid(
                "Reconstructed image header failed integrity verification",
            ));
        }
        Ok(descriptor)
    }
}
