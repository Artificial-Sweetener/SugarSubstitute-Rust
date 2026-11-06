// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render embedded vector artwork with its authored viewport and aspect ratio.
//!
//! Parsing and rasterization run on the caller's admitted preparation worker.
//! SVG parsing/filter allocation is opaque library work bounded by the embedded
//! source and viewport limits. Output storage keeps its execution memory claim.
//! No image resource, filesystem path, font discovery or network lookup is admitted.

use crate::bitmap::Bitmap;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use tiny_skia::{IntSize, Transform};

#[path = "vector_viewport.rs"]
mod viewport;
pub use viewport::VectorViewport;

/// Parsed immutable artwork; callers retain it with their preparation state.
pub struct VectorAsset {
    tree: resvg::usvg::Tree,
}

/// Owned premultiplied vector pixels and their admitted output capacity.
pub struct VectorPixels {
    pixels: Bitmap,
    origin: [i32; 2],
    // The output allocation retires before its reservation becomes reusable.
    _claim: Option<MemoryClaim>,
}

impl VectorAsset {
    /// Parse bounded embedded artwork without resolving external or inline images.
    /// The source must be a compile-time product asset, never user document content.
    /// # Errors
    /// Preserves parser failures and rejects excessive sources or image references.
    pub fn parse(source: &'static str) -> io::Result<Self> {
        if source.is_empty() || source.len() > 262_144 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid embedded vector size",
            ));
        }
        let resolved = Arc::new(AtomicBool::new(false));
        let data = resolved.clone();
        let string = resolved.clone();
        let options = resvg::usvg::Options {
            image_href_resolver: resvg::usvg::ImageHrefResolver {
                resolve_data: Box::new(move |_, _, _| {
                    data.store(true, Ordering::Relaxed);
                    None
                }),
                resolve_string: Box::new(move |_, _| {
                    string.store(true, Ordering::Relaxed);
                    None
                }),
            },
            ..Default::default()
        };
        let tree = resvg::usvg::Tree::from_str(source, &options).map_err(io::Error::other)?;
        if resolved.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Embedded vector references an image resource",
            ));
        }
        Ok(Self { tree })
    }

    /// Render into a bounded viewport, centered without stretching or cropping.
    /// Fractional device size and phase survive inside an integer pixel buffer;
    /// callers composite at the returned origin without another transform.
    /// A missing lease is reserved for synchronous offscreen qualification.
    /// # Errors
    /// Rejects invalid output geometry, memory admission and allocation failures.
    pub fn rasterize(
        &self,
        viewport: VectorViewport,
        lease: Option<MemoryLease>,
    ) -> io::Result<VectorPixels> {
        let viewport = viewport.device()?;
        let [width, height] = viewport.size;
        let bytes = width as usize * height as usize * 4;
        let claim = lease.map(|l| l.claim(bytes as u64).map_err(io::Error::other)).transpose()?;
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(bytes).map_err(io::Error::other)?;
        if buffer.capacity() != bytes {
            return Err(io::Error::other("Vector allocation exceeds admitted capacity"));
        }
        buffer.resize(bytes, 0);
        let size = IntSize::from_wh(width, height)
            .ok_or_else(|| io::Error::other("Vector size absent"))?;
        let mut pixels = Bitmap::from_vec(buffer, size)
            .ok_or_else(|| io::Error::other("Vector buffer shape changed"))?;
        let source = self.tree.size();
        let scale = (viewport.extent[0] / source.width()).min(viewport.extent[1] / source.height());
        let x = viewport.phase[0] + (viewport.extent[0] - source.width() * scale) * 0.5;
        let y = viewport.phase[1] + (viewport.extent[1] - source.height() * scale) * 0.5;
        resvg::render(
            &self.tree,
            Transform::from_row(scale, 0.0, 0.0, scale, x, y),
            &mut pixels.raw_mut(),
        );
        Ok(VectorPixels { pixels, origin: viewport.origin, _claim: claim })
    }
}

impl VectorPixels {
    /// Recolor glyph coverage in place, preserving premultiplication and its output memory claim.
    pub(crate) fn recolor(&mut self, color: [u8; 4]) {
        for pixel in self.pixels.data_mut().chunks_exact_mut(4) {
            let alpha = (u16::from(pixel[3]) * u16::from(color[3]) + 127) / 255;
            for channel in 0..3 {
                pixel[channel] = ((u16::from(color[channel]) * alpha + 127) / 255) as u8;
            }
            pixel[3] = alpha as u8;
        }
    }
    /// Integer device placement of the buffer, including its fractional edge coverage.
    /// Composite at this origin without rescaling or rounding the authored viewport.
    pub fn origin(&self) -> [i32; 2] {
        self.origin
    }
    /// Borrow immutable premultiplied RGBA output while retaining its memory claim.
    pub fn pixels(&self) -> crate::bitmap::BitmapView<'_> {
        self.pixels.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error::Error, sync::atomic::AtomicUsize};
    use sugarsubstitute_contracts::execution::MemoryRelease;

    const RECTANGLE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect x="0" y="0" width="20" height="10" fill="red"/></svg>"#;

    fn viewport(width: f32, height: f32) -> VectorViewport {
        VectorViewport { x: 0.0, y: 0.0, width, height, scale: 1.0 }
    }

    struct Release(Arc<AtomicUsize>);
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn authored_viewport_stays_centered_without_stretching() -> Result<(), Box<dyn Error>> {
        let asset = VectorAsset::parse(RECTANGLE)?;
        let pixels = asset.rasterize(viewport(40.0, 40.0), None)?;
        let view = pixels.pixels();
        for y in 0..40 {
            for x in 0..40 {
                let pixel = view.pixel(x, y).ok_or("Pixel absent")?;
                assert_eq!(pixel.alpha(), if (10..30).contains(&y) { 255 } else { 0 });
            }
        }
        Ok(())
    }

    #[test]
    fn fractional_viewport_preserves_size_phase_and_negative_placement()
    -> Result<(), Box<dyn Error>> {
        let asset = VectorAsset::parse(RECTANGLE)?;
        let pixels = asset.rasterize(
            VectorViewport { x: 10.2, y: 8.4, width: 22.0, height: 11.0, scale: 1.25 },
            None,
        )?;
        assert_eq!(pixels.origin(), [12, 10]);
        let view = pixels.pixels();
        assert_eq!((view.width(), view.height()), (29, 15));
        // Both quarter-covered edge pixels prove that 27.5 was not rounded to 28.
        for x in [0, 28] {
            assert!(view.pixel(x, 5).ok_or("Fractional edge absent")?.alpha().abs_diff(64) <= 1);
        }
        assert_eq!(view.pixel(1, 5).ok_or("Interior absent")?.alpha(), 255);
        let shifted = asset.rasterize(
            VectorViewport { x: -1.8, y: -3.6, width: 22.0, height: 11.0, scale: 1.25 },
            None,
        )?;
        assert_eq!(shifted.origin(), [-3, -5]);
        assert_eq!(shifted.pixels().data(), view.data());
        Ok(())
    }

    #[test]
    fn fractional_edge_storage_is_included_in_memory_admission() -> Result<(), Box<dyn Error>> {
        let asset = VectorAsset::parse(RECTANGLE)?;
        let viewport = VectorViewport { x: 0.5, y: 0.5, ..viewport(20.0, 20.0) };
        let lease = MemoryLease::new(20 * 20 * 4, Box::new(Release(Arc::new(AtomicUsize::new(0)))));
        assert!(asset.rasterize(viewport, Some(lease)).is_err());
        let lease = MemoryLease::new(21 * 21 * 4, Box::new(Release(Arc::new(AtomicUsize::new(0)))));
        let pixels = asset.rasterize(viewport, Some(lease.clone()))?;
        assert!(lease.claim(1).is_err());
        drop(pixels);
        assert!(lease.claim(21 * 21 * 4).is_ok());
        Ok(())
    }

    #[test]
    fn output_claim_survives_render_and_releases_with_pixels() -> Result<(), Box<dyn Error>> {
        let asset = VectorAsset::parse(RECTANGLE)?;
        let released = Arc::new(AtomicUsize::new(0));
        let lease = MemoryLease::new(1600, Box::new(Release(released.clone())));
        let pixels = asset.rasterize(viewport(20.0, 20.0), Some(lease.clone()))?;
        assert!(lease.claim(1).is_err());
        drop(pixels);
        assert!(lease.claim(1600).is_ok());
        drop(lease);
        assert_eq!(released.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn bad_geometry_and_undercharged_output_are_rejected() -> Result<(), Box<dyn Error>> {
        let asset = VectorAsset::parse(RECTANGLE)?;
        for (w, h) in [(0, 10), (10, 0), (2049, 10), (10, 2049), (u32::MAX, u32::MAX)] {
            assert!(asset.rasterize(viewport(w as f32, h as f32), None).is_err());
        }
        assert!(
            asset
                .rasterize(
                    viewport(20.0, 20.0),
                    Some(MemoryLease::new(1, Box::new(Release(Arc::new(AtomicUsize::new(0))))))
                )
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn vector_images_cannot_resolve_files_or_inline_image_data() {
        for source in [
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="C:/private.png" width="20" height="20"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="https://example.invalid/private.png" width="20" height="20"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><image href="data:image/png;base64,eA==" width="20" height="20"/></svg>"#,
        ] {
            assert!(VectorAsset::parse(source).is_err());
        }
    }
}
