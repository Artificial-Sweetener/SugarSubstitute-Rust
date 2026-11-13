// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode PNG scanlines into bounded original-precision RGBA archive stripes.
use crate::{ImportError, Packing, RasterDescriptor, Transfer, buffer::Buffer, write_raster};
use std::io::{BufRead, Seek, Write};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::Extent;

const DECODER_LIMIT: usize = 8 * 1024 * 1024;
const DECODER_RESERVATION: u64 = 16 * 1024 * 1024;
const MAX_ROW: usize = 2 * 1024 * 1024;

/// Bounded PNG header/scanline decoder owned by one image-import worker.
/// Palette, grayscale, transparency and 8/16-bit samples expand without reducing precision.
/// Noninterlaced sRGB-primary PNGs are supported; unsupported ICC, HDR primary encodings,
/// animation and interlacing return explicit errors before a derived archive is published.
/// The caller retains authoritative original bytes and admits any input buffering separately.
pub struct PngImport<R: BufRead + Seek> {
    reader: png::Reader<R>,
    descriptor: RasterDescriptor,
    color: png::ColorType,
    line_bytes: usize,
    _decoder: MemoryClaim,
}
impl<R: BufRead + Seek> PngImport<R> {
    /// Parse bounded source metadata and validate source color/sample interpretation.
    /// `content` is the SHA-256 of the exact immutable original input stream.
    /// # Errors
    /// Preserves PNG/CRC, input, admission and control failures; rejects unsupported encodings.
    pub fn open(input: R, content: [u8; 32], context: &JobContext) -> Result<Self, ImportError> {
        crate::check(context)?;
        let memory = context.memory().claim(DECODER_RESERVATION)?;
        let mut decoder =
            png::Decoder::new_with_limits(input, png::Limits { bytes: DECODER_LIMIT });
        decoder.set_transformations(png::Transformations::EXPAND);
        decoder.set_ignore_text_chunk(true);
        let reader = decoder.read_info()?;
        let info = reader.info();
        if info.interlaced || info.animation_control.is_some() {
            return Err(ImportError::Invalid(
                "PNG scanline import requires a noninterlaced still image",
            ));
        }
        if info.icc_profile.is_some() || info.coding_independent_code_points.is_some() {
            return Err(ImportError::Invalid(
                "PNG color profile requires a supported color-conversion adapter",
            ));
        }
        let srgb = png::SourceChromaticities::new(
            (0.3127, 0.3290),
            (0.6400, 0.3300),
            (0.3000, 0.6000),
            (0.1500, 0.0600),
        );
        if info.chromaticities().is_some_and(|primaries| primaries != srgb) {
            return Err(ImportError::Invalid(
                "PNG primaries require a supported color-conversion adapter",
            ));
        }
        let transfer = if info.srgb.is_some() {
            Transfer::Srgb
        } else {
            info.gamma().map_or(Transfer::Srgb, |gamma| Transfer::Gamma(gamma.into_value()))
        };
        let (color, depth) = reader.output_color_type();
        let packing = match depth {
            png::BitDepth::Eight => Packing::Rgba8,
            png::BitDepth::Sixteen => Packing::Rgba16,
            _ => {
                return Err(ImportError::Invalid(
                    "PNG sample expansion did not produce complete bytes",
                ));
            }
        };
        let line_bytes = reader
            .output_line_size(info.width)
            .filter(|bytes| *bytes <= MAX_ROW)
            .ok_or(ImportError::Invalid("PNG scanline exceeds bounded decoder workspace"))?;
        let descriptor = RasterDescriptor::new(
            Extent::new(info.width, info.height)?,
            packing,
            transfer,
            content,
        )?;
        crate::check(context)?;
        Ok(Self { reader, descriptor, color, line_bytes, _decoder: memory })
    }
    /// Validated original interpretation used for cache admission and exact source identities.
    pub const fn descriptor(&self) -> RasterDescriptor {
        self.descriptor
    }
    /// Stream all decoded rows and validate the PNG terminator before returning success.
    /// Storage publication belongs to the caller after this operation and its original controls succeed.
    /// # Errors
    /// Preserves source truncation/CRC, allocation, cancellation, admission and write failures.
    pub fn write(
        mut self,
        output: &mut dyn Write,
        context: &JobContext,
    ) -> Result<(), ImportError> {
        let mut line = Buffer::new(self.line_bytes, 0, context.memory())?;
        let color = self.color;
        let sample_bytes = self.descriptor.layout().packing().bytes() as usize / 4;
        write_raster(output, self.descriptor, context, |_, target| {
            if self.reader.read_row(&mut line.data)?.is_none() {
                return Err(ImportError::Invalid("PNG ended before its declared image extent"));
            }
            expand_row(color, sample_bytes, &line.data, target)
        })?;
        if self.reader.read_row(&mut line.data)?.is_some() {
            return Err(ImportError::Invalid("PNG contains an unexpected image row"));
        }
        self.reader.finish()?;
        crate::check(context)
    }
}

/// Preserve source sample bytes while expanding absent channels and straight alpha.
fn expand_row(
    color: png::ColorType,
    sample: usize,
    source: &[u8],
    target: &mut [u8],
) -> Result<(), ImportError> {
    let channels = match color {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(ImportError::Invalid("PNG palette expansion remained indexed"));
        }
    };
    if target.len() / 4 / sample * channels * sample != source.len() {
        return Err(ImportError::Invalid("PNG row dimensions differ from expanded source"));
    }
    if channels == 4 {
        target.copy_from_slice(source);
        return Ok(());
    }
    for (input, output) in
        source.chunks_exact(channels * sample).zip(target.chunks_exact_mut(4 * sample))
    {
        if channels <= 2 {
            for channel in 0..3 {
                output[channel * sample..(channel + 1) * sample].copy_from_slice(&input[..sample]);
            }
        } else {
            output[..3 * sample].copy_from_slice(input);
        }
        if channels == 2 {
            output[3 * sample..].copy_from_slice(&input[sample..]);
        } else {
            output[3 * sample..].fill(255);
        }
    }
    Ok(())
}
