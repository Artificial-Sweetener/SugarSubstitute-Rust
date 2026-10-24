// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Owned premultiplied pixels and borrowed views across the shared painting boundary.
//!
//! Callers retain execution admission for allocations and painting. Backing raster
//! objects stay private; these values carry pixel data and never filesystem authority.
use std::io;
use tiny_skia::{
    Color, FillRule, IntSize, Paint, Path, PixmapPaint, PremultipliedColorU8, Rect, Transform,
};
mod mask;
pub use mask::CoverageMask;

/// Owned RGBA pixels. The caller keeps their original memory claim through destruction.
/// ```compile_fail
/// use sugarsubstitute_ui::bitmap::Bitmap;
/// fn bypass(image: Bitmap) { let raw_image = image.0; }
/// ```
#[derive(Clone, PartialEq)]
pub struct Bitmap(tiny_skia::Pixmap);

/// Borrowed RGBA pixels; no raw backing image or path-based output can escape the borrow.
#[derive(Clone, Copy)]
pub struct BitmapView<'a>(tiny_skia::PixmapRef<'a>);

impl Bitmap {
    /// Allocate zeroed pixels after the caller has admitted their required capacity.
    /// Returns `None` for zero, overflowing or unsupported dimensions.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        tiny_skia::Pixmap::new(width, height).map(Self)
    }
    /// Adopt an existing admitted RGBA allocation without copying or changing its capacity.
    /// Returns `None` when the byte count does not match the dimensions.
    pub fn from_vec(bytes: Vec<u8>, size: IntSize) -> Option<Self> {
        tiny_skia::Pixmap::from_vec(bytes, size).map(Self)
    }
    /// Decode in-memory qualification bytes; production asset decoders retain their own bounds.
    /// # Errors
    /// Preserves invalid PNG data and decoder failures without acquiring any path.
    pub fn decode_png(bytes: &[u8]) -> io::Result<Self> {
        tiny_skia::Pixmap::decode_png(bytes).map(Self).map_err(io::Error::other)
    }
    /// Encode pixels into owned bytes for publication through the storage owner.
    /// # Errors
    /// Preserves the in-memory encoder failure.
    pub fn encode_png(&self) -> io::Result<Vec<u8>> {
        self.0.encode_png().map_err(io::Error::other)
    }
    /// Borrow a data-only image view without releasing the original memory custody.
    pub fn as_ref(&self) -> BitmapView<'_> {
        BitmapView(self.0.as_ref())
    }
    /// Physical image width in pixels.
    pub fn width(&self) -> u32 {
        self.0.width()
    }
    /// Physical image height in pixels.
    pub fn height(&self) -> u32 {
        self.0.height()
    }
    /// Borrow premultiplied RGBA bytes in row-major order.
    pub fn data(&self) -> &[u8] {
        self.0.data()
    }
    /// Borrow the original RGBA allocation for bounded caller-owned painting.
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.0.data_mut()
    }
    /// Inspect one pixel, rejecting coordinates outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<PremultipliedColorU8> {
        (x < self.width() && y < self.height()).then(|| self.0.pixel(x, y)).flatten()
    }
    /// Borrow the image's premultiplied color values.
    pub fn pixels(&self) -> &[PremultipliedColorU8] {
        self.0.pixels()
    }
    /// Borrow mutable premultiplied color values within the original allocation.
    pub fn pixels_mut(&mut self) -> &mut [PremultipliedColorU8] {
        self.0.pixels_mut()
    }
    /// Transfer the exact RGBA allocation; the caller transfers its admission alongside it.
    pub fn take(self) -> Vec<u8> {
        self.0.take()
    }
    /// Fill existing pixels with a premultiplied color without resizing the image.
    pub fn fill(&mut self, color: Color) {
        self.0.fill(color);
    }
    /// Paint a rectangle into the retained image with an optional data-only coverage mask.
    pub fn fill_rect(
        &mut self,
        rect: Rect,
        paint: &Paint<'_>,
        transform: Transform,
        mask: Option<&CoverageMask>,
    ) {
        self.0.fill_rect(rect, paint, transform, mask.map(CoverageMask::raw));
    }
    /// Paint a filled path with the original rasterizer's winding, clipping and transform rules.
    pub fn fill_path(
        &mut self,
        path: &Path,
        paint: &Paint<'_>,
        rule: FillRule,
        transform: Transform,
        mask: Option<&CoverageMask>,
    ) {
        self.0.fill_path(path, paint, rule, transform, mask.map(CoverageMask::raw));
    }
    /// Composite borrowed pixels without granting either image any filesystem capability.
    pub fn draw_bitmap(
        &mut self,
        x: i32,
        y: i32,
        image: BitmapView<'_>,
        paint: &PixmapPaint,
        transform: Transform,
        mask: Option<&CoverageMask>,
    ) {
        self.0.draw_pixmap(x, y, image.0, paint, transform, mask.map(CoverageMask::raw));
    }
    /// Lend backing pixels only inside the shared owner for the admitted SVG renderer.
    pub(crate) fn raw_mut(&mut self) -> tiny_skia::PixmapMut<'_> {
        self.0.as_mut()
    }
}

impl<'a> BitmapView<'a> {
    /// Validate a borrowed RGBA layout without allocating or taking ownership of its bytes.
    pub fn from_bytes(bytes: &'a [u8], width: u32, height: u32) -> Option<Self> {
        tiny_skia::PixmapRef::from_bytes(bytes, width, height).map(Self)
    }
    /// Physical image width in pixels.
    pub fn width(self) -> u32 {
        self.0.width()
    }
    /// Physical image height in pixels.
    pub fn height(self) -> u32 {
        self.0.height()
    }
    /// Borrow the exact source bytes for the original view lifetime.
    pub fn data(self) -> &'a [u8] {
        self.0.data()
    }
    /// Inspect an existing pixel without reading beyond the borrowed image.
    pub fn pixel(self, x: u32, y: u32) -> Option<PremultipliedColorU8> {
        (x < self.width() && y < self.height()).then(|| self.0.pixel(x, y)).flatten()
    }
    /// Copy pixels only after the caller admits the independent output allocation.
    pub fn to_owned(self) -> Bitmap {
        Bitmap(self.0.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adopted_pixels_and_views_preserve_original_allocation_and_bounds() -> io::Result<()> {
        let mut bytes = vec![0; 16];
        bytes[0..4].copy_from_slice(&[10, 20, 30, 255]);
        let pointer = bytes.as_ptr();
        let size = IntSize::from_wh(2, 2).ok_or_else(|| io::Error::other("Invalid test size"))?;
        let image =
            Bitmap::from_vec(bytes, size).ok_or_else(|| io::Error::other("Invalid image"))?;
        assert_eq!(image.data().as_ptr(), pointer);
        let view = image.as_ref();
        assert_eq!((view.width(), view.height()), (2, 2));
        assert_eq!(view.data(), image.data());
        for (x, y) in [(2, 0), (0, 2), (u32::MAX, 0), (0, u32::MAX)] {
            assert!(view.pixel(x, y).is_none());
            assert!(image.pixel(x, y).is_none());
        }
        let transferred = image.take();
        assert_eq!(transferred.as_ptr(), pointer);
        assert!(BitmapView::from_bytes(&[0; 3], 1, 1).is_none());
        Ok(())
    }
    #[test]
    fn data_only_composition_preserves_actual_raster_pixels() -> io::Result<()> {
        let source = Bitmap::new(3, 3).ok_or_else(|| io::Error::other("Invalid source"))?;
        let mut safe = Bitmap::new(8, 8).ok_or_else(|| io::Error::other("Invalid destination"))?;
        let mut reference =
            tiny_skia::Pixmap::new(8, 8).ok_or_else(|| io::Error::other("Invalid reference"))?;
        let mut source = source;
        source.fill(Color::from_rgba8(72, 45, 191, 131));
        safe.draw_bitmap(
            2,
            1,
            source.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        reference.draw_pixmap(
            2,
            1,
            source.0.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        assert_eq!(safe.data(), reference.data());
        Ok(())
    }
}
