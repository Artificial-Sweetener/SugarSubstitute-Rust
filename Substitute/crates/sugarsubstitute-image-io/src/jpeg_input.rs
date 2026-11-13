// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode reviewed sequential JPEG samples into the existing sRGB raster interpretation.
mod control;
mod header;
use crate::{ImportError, Packing, RasterDescriptor, Transfer, buffer::Buffer, write_raster};
use control::CheckedInput;
use header::Header;
use std::io::{BufRead, Seek, Write};
use sugarsubstitute_contracts::execution::JobContext;
use zune_core::{colorspace::ColorSpace, options::DecoderOptions};
use zune_jpeg::JpegDecoder;

/// Bounded sequential 8-bit JPEG import with original-byte identity and explicit admission.
///
/// Supports grayscale and RGB 1x1, and YCbCr 4:4:4/4:2:2/4:2:0 with one complete sequential
/// scan. JFIF is the only accepted metadata. Profiles, orientation metadata, HDR, progressive,
/// separated-component scans and restart markers require another reviewed interpretation.
/// Scalar zune-jpeg 0.5.15 RGB samples, opaque alpha and sRGB transfer define this initial
/// cache interpretation. An incompatible decoder change requires deliberate cache compatibility.
/// Dimensions and DC quantization must keep the worst possible baseline predictor's
/// dequantization within i32; rejected headers receive an explicit unsupported-subset error.
///
/// Controls are checked at every input operation and before/after decoding. The public decoder
/// exposes no computation callback: one complete admitted frame is the worst-case cancellation
/// block, including malformed early-EOI input. No per-row interruption is promised.
pub struct JpegImport<R: BufRead + Seek> {
    input: R,
    header: Header,
    descriptor: RasterDescriptor,
}
impl<R: BufRead + Seek> JpegImport<R> {
    /// Validate complete bounded framing and supported source interpretation before decoder allocation.
    /// `content` is the SHA-256 of the same immutable original retained by the caller.
    /// # Errors
    /// Rejects unsupported headers, malformed framing, source limits and original controls.
    pub fn open(input: R, content: [u8; 32], context: &JobContext) -> Result<Self, ImportError> {
        let mut checked = CheckedInput::new(input, context);
        let inspected = Header::read(&mut checked);
        let rewound = checked.rewind();
        let input = checked.finish()?;
        let header = inspected?;
        rewound?;
        let descriptor =
            RasterDescriptor::new(header.extent, Packing::Rgba8, Transfer::Srgb, content)?;
        Ok(Self { input, header, descriptor })
    }
    /// Validated original interpretation used by shared source-cache admission.
    pub const fn descriptor(&self) -> RasterDescriptor {
        self.descriptor
    }

    /// Admit decoded RGB and reviewed decoder scratch before invoking the decoder.
    /// The decoder retires before archive stripe/chunk buffers are allocated.
    /// # Errors
    /// Preserves decoder, allocation, admission, input, cancellation and output failures.
    pub fn write(self, output: &mut dyn Write, context: &JobContext) -> Result<(), ImportError> {
        crate::check(context)?;
        let claim = context.memory().claim(self.header.decoder_bytes()?)?;
        let mut rgb = Buffer::new(self.header.rgb_bytes()?, 0, context.memory())?;
        let options = DecoderOptions::new_safe()
            .set_strict_mode(true)
            .set_max_width(self.header.extent.width() as usize)
            .set_max_height(self.header.extent.height() as usize)
            .jpeg_set_out_colorspace(ColorSpace::RGB);
        let mut checked = CheckedInput::new(self.input, context);
        let mut decoder = JpegDecoder::new_with_options(&mut checked, options);
        let decoded = (|| {
            decoder.decode_headers()?;
            if decoder.output_buffer_size() != Some(rgb.data.len()) {
                return Err(ImportError::Invalid("JPEG decoder dimensions differ from preflight"));
            }
            decoder.decode_into(&mut rgb.data)?;
            Ok(())
        })();
        drop(decoder);
        let controlled = checked.finish();
        drop(claim);
        controlled?;
        decoded?;
        crate::check(context)?;
        let width = self.header.extent.width() as usize;
        write_raster(output, self.descriptor, context, |y, target| {
            let start = y as usize * width * 3;
            for (source, rgba) in
                rgb.data[start..start + width * 3].chunks_exact(3).zip(target.chunks_exact_mut(4))
            {
                rgba[..3].copy_from_slice(source);
                rgba[3] = 255;
            }
            Ok(())
        })
    }
}
