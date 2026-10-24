// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep renderer pixels and in-memory image encoding independent of filesystem acquisition.
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// Own tightly packed, top-row-first premultiplied RGBA8 device pixels.
/// Painting stays with the character and splash owners. Borrowed pixels cannot outlive this
/// raster; encoded PNG bytes transfer to the caller's output-storage owner.
pub struct Raster {
    pub(crate) pixels: Pixmap,
}

impl Raster {
    /// Allocate transparent device pixels with each nonzero dimension at most 4096.
    /// Returns `None` for invalid geometry, matching the splash's supported frame bounds.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        if width > 4096 || height > 4096 {
            return None;
        }
        Pixmap::new(width, height).map(|pixels| Self { pixels })
    }

    /// Return the device-pixel width.
    pub fn width(&self) -> u32 {
        self.pixels.width()
    }

    /// Return the device-pixel height.
    pub fn height(&self) -> u32 {
        self.pixels.height()
    }

    /// Borrow tightly packed premultiplied RGBA8 bytes, with four bytes per pixel.
    pub fn data(&self) -> &[u8] {
        self.pixels.data()
    }

    /// Borrow the same storage as premultiplied channel values.
    pub fn pixels(&self) -> &[PremultipliedColorU8] {
        self.pixels.pixels()
    }

    /// Read a device pixel, returning `None` outside either dimension.
    pub fn pixel(&self, x: u32, y: u32) -> Option<PremultipliedColorU8> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        self.pixels.pixel(x, y)
    }

    /// Clear this painting target to transparent premultiplied black.
    pub fn clear(&mut self) {
        self.pixels.fill(tiny_skia::Color::TRANSPARENT);
    }

    /// Encode a PNG in memory, preserving the renderer's original demultiplication rules.
    /// # Errors
    /// Preserves PNG encoder failures. The caller owns storage and publication of the result.
    pub fn encode_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut bytes = Vec::new();
        self.write_png(&mut bytes)?;
        Ok(bytes)
    }

    /// Encode PNG bytes into a caller-owned bounded writer without a second encoded buffer.
    /// # Errors
    /// Preserves allocation, encoder and destination-write failures. The raster remains owned
    /// throughout conversion; storage authority and publication stay with the supplied writer.
    pub fn write_png(
        &self,
        output: &mut (impl std::io::Write + ?Sized),
    ) -> Result<(), png::EncodingError> {
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(self.data().len())
            .map_err(|cause| png::EncodingError::IoError(std::io::Error::other(cause)))?;
        for pixel in self.pixels() {
            let color = pixel.demultiply();
            rgba.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
        }
        let mut encoder = png::Encoder::new(output, self.width(), self.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba)?;
        writer.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Streamed artifacts retain the original codec's exact alpha rounding and error chain.
    #[test]
    fn streamed_png_preserves_original_variable_alpha_bytes_and_writer_failures()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut source = Pixmap::new(17, 17).ok_or("Raster allocation failed")?;
        for (index, pixel) in source.data_mut().chunks_exact_mut(4).enumerate() {
            let alpha = (index % 256) as u8;
            pixel.copy_from_slice(&[alpha / 2, alpha / 3, alpha / 4, alpha]);
        }
        let expected = source.encode_png()?;
        let raster = Raster { pixels: source };
        let mut actual = Vec::new();
        raster.write_png(&mut actual)?;
        assert_eq!(actual, expected);
        struct Rejected;
        impl std::io::Write for Rejected {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "selected writer rejected",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let cause = raster.write_png(&mut Rejected).err().ok_or("Writer rejection disappeared")?;
        assert!(
            matches!(cause, png::EncodingError::IoError(ref source) if source.kind() == std::io::ErrorKind::PermissionDenied)
        );
        struct TailRejected(usize);
        impl std::io::Write for TailRejected {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.0 == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "PNG trailer rejected",
                    ));
                }
                let count = self.0.min(bytes.len());
                self.0 -= count;
                Ok(count)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(raster.write_png(&mut TailRejected(expected.len() - 1)).is_err());
        Ok(())
    }

    /// Pixel borrowing, edge rejection and PNG encoding preserve actual native raster bytes.
    #[test]
    fn raster_preserves_pixels_and_png_encoding_without_exporting_the_backing_library()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut source = Pixmap::new(3, 2).ok_or("Raster allocation failed")?;
        source.fill(tiny_skia::Color::from_rgba8(211, 89, 37, 173));
        let mut raster = Raster { pixels: source.clone() };
        assert_eq!((raster.width(), raster.height()), (3, 2));
        assert_eq!(raster.data(), source.data());
        assert_eq!(raster.pixel(2, 1), source.pixel(2, 1));
        assert_eq!(raster.pixel(3, 0), None);
        assert_eq!(raster.pixel(0, 2), None);
        assert_eq!(raster.encode_png()?, source.encode_png()?);
        let decoded = Pixmap::decode_png(&raster.encode_png()?)?;
        assert_eq!((decoded.width(), decoded.height()), (3, 2));
        raster.clear();
        assert!(raster.data().iter().all(|byte| *byte == 0));
        assert!(Raster::new(0, 1).is_none());
        assert!(Raster::new(1, 4097).is_none());
        Ok(())
    }
}
