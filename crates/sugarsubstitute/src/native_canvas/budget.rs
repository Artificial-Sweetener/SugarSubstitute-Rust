// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund canvas storage for the largest canvas the attached displays can show.
//!
//! One admitted canvas reservation is split in two. Presentation storage funds the graphics
//! device, composition surfaces and shared textures that the UI thread claims, where cached
//! tiles cannot be evicted, so tile caching can never starve it. Retained storage funds
//! everything prepared on workers, whose claims evict cached tiles under pressure. Both are
//! admission ceilings, not allocations, sized for what can coexist at the largest extent.

use super::coverage::DETAIL_MARGIN;
use sugarsubstitute_canvas::MAX_SURFACE_EDGE;
use sugarsubstitute_image_ops::{Extent, ImageError, Pixel, TILE_EDGE};

/// Cached graph tiles each canvas renderer may retain.
pub(super) const RENDERER_TILES: usize = 512;
/// Recently shown output documents an embedded canvas keeps for instant reselection.
#[cfg(feature = "native-composition")]
pub(super) const RETAINED_DOCUMENTS: usize = 16;
/// Fixed storage an embedded canvas funds for its retained output documents' backings,
/// independent of the display extent. Retention evicts the least recently shown document
/// before its ledger exceeds this allowance.
#[cfg(any(all(windows, feature = "windows-native-host"), test))]
pub(crate) const RETAINED_DOCUMENT_BYTES: u64 = 512 * 1024 * 1024;
/// Document, history, sampling tables and engine metadata independent of canvas size.
const FIXED_BYTES: u64 = 16 * 1024 * 1024;
/// Margin-extended canonical surfaces alive together: three document tiers, a replacement in
/// preparation and its snapshot, and a preview candidate's backing and replacement.
const CANONICAL_SURFACES: u64 = 7;
/// Viewport-sized display buffers: the document's pair, a replacement pair admitted during a
/// resize and a preview candidate's pair.
const DISPLAY_BUFFERS: u64 = 6;
/// Margin-extended GPU display images in GPU sessions: three tiers and a replacement.
const GPU_SURFACES: u64 = 4;
/// Graphics device and drawing context storage.
const DEVICE_BYTES: u64 = 8 * 1024 * 1024;
/// Composition surfaces: the shown one, its resize replacement and two awaiting compositor
/// retirement. Each accounts three pixel copies and 1 MiB of driver overhead.
const COMPOSITION_SURFACES: u64 = 4;
const COMPOSITION_PIXEL_BYTES: u64 = 12;
const COMPOSITION_OVERHEAD_BYTES: u64 = 1024 * 1024;
/// Shared GPU frame textures in GPU sessions, each with 64 KiB of synchronization overhead.
const SHARED_TEXTURES: u64 = 3;
const SHARED_TEXTURE_OVERHEAD_BYTES: u64 = 64 * 1024;
/// Largest canvas funded when the attached displays cannot be measured: one 4K display.
const FALLBACK_EXTENT: [u32; 2] = [3840, 2160];

/// Largest physical canvas extent the attached displays can show, measured once per call.
/// An unmeasurable topology is logged and funds one 4K display instead.
/// # Errors
/// Preserves extent validation failures; clamped edges are always valid.
pub(crate) fn largest_canvas_extent() -> Result<Extent, ImageError> {
    #[cfg(windows)]
    let measured = sugarsubstitute_desktop::canvas::desktop_extent()
        .inspect_err(|cause| {
            tracing::warn!(
                operation = "canvas_desktop_extent",
                cause = %cause,
                "Attached displays could not be measured; funding a 4K canvas"
            );
        })
        .ok();
    #[cfg(not(windows))]
    let measured = None;
    let [width, height] = measured.unwrap_or(FALLBACK_EXTENT);
    let clamp = |edge: u32| edge.clamp(1, MAX_SURFACE_EDGE);
    Extent::new(clamp(width), clamp(height))
}

/// Whole canvas reservation for a session whose canvas may reach `maximum` physical pixels and
/// which retains up to `retained_documents` bytes of recently shown document backings.
pub(crate) fn reservation_bytes(maximum: Extent, gpu: bool, retained_documents: u64) -> u64 {
    retained_bytes(maximum, gpu)
        .saturating_add(retained_documents)
        .saturating_add(presentation_bytes(maximum, gpu))
}

/// Worker-prepared storage: one renderer's tile cache plus every coexisting canonical surface
/// and display buffer at `maximum`. GPU sessions add a device tile cache and display images.
pub(super) fn retained_bytes(maximum: Extent, gpu: bool) -> u64 {
    let viewport = maximum.pixels();
    let extended = margin_extended(maximum);
    let tiles = RENDERER_TILES as u64
        * (u64::from(TILE_EDGE) * u64::from(TILE_EDGE) * size_of::<Pixel>() as u64 + 128);
    let mut bytes = FIXED_BYTES
        .saturating_add(tiles)
        .saturating_add(extended.saturating_mul(CANONICAL_SURFACES * 4))
        .saturating_add(viewport.saturating_mul(DISPLAY_BUFFERS * 4));
    if gpu {
        bytes =
            bytes.saturating_add(tiles).saturating_add(extended.saturating_mul(GPU_SURFACES * 4));
    }
    bytes
}

/// UI-thread presentation storage: the graphics device, every coexisting composition surface
/// and, in GPU sessions, the shared frame textures at `maximum`.
pub(super) fn presentation_bytes(maximum: Extent, gpu: bool) -> u64 {
    let viewport = maximum.pixels();
    let surface =
        viewport.saturating_mul(COMPOSITION_PIXEL_BYTES).saturating_add(COMPOSITION_OVERHEAD_BYTES);
    let mut bytes = DEVICE_BYTES.saturating_add(surface.saturating_mul(COMPOSITION_SURFACES));
    if gpu {
        let texture = viewport.saturating_mul(4).saturating_add(SHARED_TEXTURE_OVERHEAD_BYTES);
        bytes = bytes.saturating_add(texture.saturating_mul(SHARED_TEXTURES));
    }
    bytes
}

/// Pixels in a canonical surface covering `viewport` plus its detail margin on every side.
fn margin_extended(viewport: Extent) -> u64 {
    let edge = |edge: u32, margin: u32| u64::from(edge) + 2 * u64::from(margin);
    edge(viewport.width(), DETAIL_MARGIN[0])
        .saturating_mul(edge(viewport.height(), DETAIL_MARGIN[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1024 * 1024;

    #[test]
    fn presentation_funds_every_coexisting_composition_surface_at_the_largest_canvas()
    -> Result<(), Box<dyn std::error::Error>> {
        let ultrawide = Extent::new(3440, 1440)?;
        // The shown surface, its resize replacement and two awaiting retirement, as claimed by
        // the composition owner, fit beside the graphics device.
        let surface = ultrawide.pixels() * 4 * 3 + MIB;
        assert!(presentation_bytes(ultrawide, false) >= 4 * surface + DEVICE_BYTES);
        let shared = 3 * (ultrawide.pixels() * 4 + 64 * 1024);
        assert!(
            presentation_bytes(ultrawide, true) >= presentation_bytes(ultrawide, false) + shared
        );
        Ok(())
    }

    #[test]
    fn retained_storage_keeps_the_small_canvas_floor_and_grows_with_the_display()
    -> Result<(), Box<dyn std::error::Error>> {
        // A tiny canvas fits its working reservation within the original 160 MiB CPU budget.
        let tiny = retained_bytes(Extent::new(1, 1)?, false);
        assert!(reservation_bytes(Extent::new(1, 1)?, false, 0) <= 160 * MIB);
        let ultrawide = Extent::new(3440, 1440)?;
        let cpu = retained_bytes(ultrawide, false);
        // Growth is exactly the seven margin-extended canonical surfaces and six display buffers.
        let per_canvas = |[width, height]: [u64; 2]| {
            7 * (width + 256) * (height + 256) * 4 + 6 * width * height * 4
        };
        assert_eq!(cpu - tiny, per_canvas([3440, 1440]) - per_canvas([1, 1]));
        assert!(retained_bytes(ultrawide, true) > cpu);
        Ok(())
    }

    #[test]
    fn retained_documents_add_their_fixed_allowance_at_every_extent()
    -> Result<(), Box<dyn std::error::Error>> {
        for extent in [Extent::new(1, 1)?, Extent::new(3840, 2160)?] {
            for gpu in [false, true] {
                assert_eq!(
                    reservation_bytes(extent, gpu, RETAINED_DOCUMENT_BYTES)
                        - reservation_bytes(extent, gpu, 0),
                    512 * MIB
                );
            }
        }
        // The allowance holds a dozen full-quality 4K detail backings with their fallbacks.
        let detail = margin_extended(Extent::new(3840, 2160)?) * 4;
        assert!(RETAINED_DOCUMENT_BYTES >= 12 * (detail + 1024 * 1024 * 4));
        Ok(())
    }

    #[test]
    fn reservations_saturate_instead_of_overflowing() -> Result<(), Box<dyn std::error::Error>> {
        let largest = i32::MAX as u32;
        assert_eq!(reservation_bytes(Extent::new(largest, largest)?, true, 0), u64::MAX);
        let small = reservation_bytes(Extent::new(1280, 800)?, false, RETAINED_DOCUMENT_BYTES);
        let fallback = Extent::new(FALLBACK_EXTENT[0], FALLBACK_EXTENT[1])?;
        assert!(small < reservation_bytes(fallback, false, RETAINED_DOCUMENT_BYTES));
        Ok(())
    }
}
