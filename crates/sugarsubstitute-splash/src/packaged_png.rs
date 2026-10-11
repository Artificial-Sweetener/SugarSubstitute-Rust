// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Decode one reviewed noninterlaced RGBA PNG through bounded row operations.

use std::{error::Error, fmt, io::Cursor};
use tiny_skia::{IntSize, Pixmap};

/// Bound packaged input/decoder scratch independently of retained decoded cache pixels.
pub(crate) const DECODER_LIMIT: usize = 1024 * 1024;

/// Preserve decoder/allocator causes and reject a changed packaged representation.
#[derive(Debug)]
pub(crate) enum PngError {
    Decode(png::DecodingError),
    Allocation(std::collections::TryReserveError),
    Format,
    Size { width: u32, height: u32 },
    Incomplete,
}
impl fmt::Display for PngError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
            Self::Format => formatter.write_str("Packaged PNG must be noninterlaced RGBA8"),
            Self::Size { width, height } => {
                write!(formatter, "Invalid packaged PNG size {width}×{height}")
            }
            Self::Incomplete => formatter.write_str("Packaged PNG decoding is incomplete"),
        }
    }
}
impl Error for PngError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::Allocation(error) => Some(error),
            _ => None,
        }
    }
}

/// Keep reader/output ownership through yields without rescanning or copying an image.
pub(crate) struct PngRows {
    reader: png::Reader<Cursor<&'static [u8]>>,
    size: IntSize,
    pixels: Vec<u8>,
    row: u32,
    finished: bool,
}
impl PngRows {
    /// Check exact dimensions and reviewed format before reserving decoded storage.
    ///
    /// Header/CRC operations are indivisible library calls over at most one MiB of
    /// compiled input. Each later step reads one output row of at most 1544 bytes.
    pub(crate) fn new(bytes: &'static [u8], width: u32, height: u32) -> Result<Self, PngError> {
        if bytes.len() > DECODER_LIMIT || width > 386 || height > 386 {
            return Err(PngError::Format);
        }
        let size = IntSize::from_wh(width, height).ok_or(PngError::Size { width, height })?;
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder.set_limits(png::Limits { bytes: DECODER_LIMIT });
        let reader = decoder.read_info().map_err(PngError::Decode)?;
        let info = reader.info();
        if info.width != width || info.height != height {
            return Err(PngError::Size { width: info.width, height: info.height });
        }
        if info.color_type != png::ColorType::Rgba
            || info.bit_depth != png::BitDepth::Eight
            || info.interlaced
            || info.animation_control.is_some()
        {
            return Err(PngError::Format);
        }
        let length = width as usize * height as usize * 4;
        if reader.output_buffer_size() != Some(length) {
            return Err(PngError::Format);
        }
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(length).map_err(PngError::Allocation)?;
        if pixels.capacity() != length {
            return Err(PngError::Format);
        }
        Ok(Self { reader, size, pixels, row: 0, finished: false })
    }

    /// Decode and premultiply one row, then finish bounded CRC parsing in its own step.
    pub(crate) fn step(&mut self) -> Result<bool, PngError> {
        if self.row < self.size.height() {
            let row =
                self.reader.next_row().map_err(PngError::Decode)?.ok_or(PngError::Incomplete)?;
            if row.data().len() != self.size.width() as usize * 4 {
                return Err(PngError::Format);
            }
            let start = self.pixels.len();
            self.pixels.extend_from_slice(row.data());
            for pixel in self.pixels[start..].chunks_exact_mut(4) {
                let alpha = pixel[3];
                for channel in &mut pixel[..3] {
                    *channel = crate::shapes::byte_product(*channel, alpha);
                }
            }
            self.row += 1;
            return Ok(false);
        }
        if !self.finished {
            self.reader.finish().map_err(PngError::Decode)?;
            self.finished = true;
        }
        Ok(true)
    }

    /// Move fully decoded pixels into a pixmap without allocating, copying or rescanning.
    pub(crate) fn finish(self) -> Result<Pixmap, PngError> {
        if !self.finished || self.row != self.size.height() {
            return Err(PngError::Incomplete);
        }
        Pixmap::from_vec(self.pixels, self.size).ok_or(PngError::Format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Cold cache construction initializes exactly one reviewed output row per step.
    #[test]
    fn each_decode_step_retains_only_one_additional_row() -> Result<(), Box<dyn Error>> {
        let bytes = crate::assets::POSE_ASSETS[0].bytes;
        let mut decoder = PngRows::new(bytes, 386, 386)?;
        assert!(decoder.pixels.is_empty());
        for row in 0..386 {
            assert!(!decoder.step()?);
            assert_eq!(decoder.row, row + 1);
            assert_eq!(decoder.pixels.len(), (row as usize + 1) * 386 * 4);
        }
        assert!(decoder.step()?);
        assert_eq!(decoder.finish()?.data().len(), 386 * 386 * 4);
        Ok(())
    }
    /// Changed resource dimensions fail before any pixel buffer becomes usable.
    #[test]
    fn dimensions_and_unfinished_decoder_are_rejected() -> Result<(), Box<dyn Error>> {
        let bytes = crate::assets::POSE_ASSETS[0].bytes;
        assert!(matches!(
            PngRows::new(bytes, 385, 386),
            Err(PngError::Size { width: 386, height: 386 })
        ));
        assert!(matches!(PngRows::new(bytes, 386, 386)?.finish(), Err(PngError::Incomplete)));
        assert!(PngRows::new(&[1, 2, 3], 386, 386).is_err());
        Ok(())
    }
}
