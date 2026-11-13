// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select one source decoder without duplicating original acquisition or cache publication.
use crate::{ImportError, JpegImport, PngImport, RasterDescriptor};
use std::io::{BufRead, Seek, Write};
use sugarsubstitute_contracts::execution::JobContext;

/// Validated original interpretation with its single decoder owner.
#[expect(
    clippy::large_enum_variant,
    reason = "Keep the single decoder inline without a new heap allocation or admission lifetime"
)]
pub(crate) enum ImageImport<R: BufRead + Seek> {
    Png(PngImport<R>),
    Jpeg(JpegImport<R>),
}
impl<R: BufRead + Seek> ImageImport<R> {
    /// Inspect bounded magic bytes on the same stream subsequently decoded and hashed.
    pub(crate) fn open(
        mut input: R,
        content: [u8; 32],
        context: &JobContext,
    ) -> Result<Self, ImportError> {
        crate::check(context)?;
        let mut signature = [0; 8];
        let mut filled = 0;
        while filled < signature.len() {
            crate::check(context)?;
            let read = input.read(&mut signature[filled..])?;
            if read == 0 {
                return Err(ImportError::Invalid("Image signature is truncated"));
            }
            filled += read;
        }
        input.rewind()?;
        crate::check(context)?;
        if signature == *b"\x89PNG\r\n\x1a\n" {
            Ok(Self::Png(PngImport::open(input, content, context)?))
        } else if signature[..2] == [255, 216] {
            Ok(Self::Jpeg(JpegImport::open(input, content, context)?))
        } else {
            Err(ImportError::Invalid("Unsupported image signature"))
        }
    }
    /// The shared cache identity remains independent of the selected decoder variant.
    pub(crate) fn descriptor(&self) -> RasterDescriptor {
        match self {
            Self::Png(import) => import.descriptor(),
            Self::Jpeg(import) => import.descriptor(),
        }
    }
    /// Reconstruct through the existing authenticated raster writer.
    pub(crate) fn write(
        self,
        output: &mut dyn Write,
        context: &JobContext,
    ) -> Result<(), ImportError> {
        match self {
            Self::Png(import) => import.write(output, context),
            Self::Jpeg(import) => import.write(output, context),
        }
    }
}
