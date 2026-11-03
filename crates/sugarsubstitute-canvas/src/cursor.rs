// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Semantic cursor feedback that tools request and native hosts render at display DPI.

/// Native cursor appearance requested by the interaction owner.
/// This changes appearance only: physical pointer position remains owned by the operating system.
/// Hosts publish this feedback after input without waiting for image work or presentation, and
/// render custom artwork at the display's DPI.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CursorIntent {
    /// The platform arrow: nothing to do here, or a tool that has no target.
    Arrow,
    /// Navigation can capture a pan gesture.
    OpenHand,
    /// Navigation currently owns a pan gesture.
    ClosedHand,
    /// A brush footprint outline centred on the pointer.
    BrushOutline(BrushOutline),
    /// A crosshair for exact placement, such as drawing a replacing selection.
    Precise,
    /// A crosshair marked as adding coverage, such as a Shift selection.
    PreciseAdd,
    /// A crosshair marked as removing coverage, such as an Alt selection.
    PreciseSubtract,
    /// The pointer is over a selection outline it can drag.
    SelectionTranslate,
    /// The pointer can move layer content.
    Move,
    /// The pointer moves selected content out of its layer, leaving the area behind empty.
    MoveCut,
    /// The tool has nothing it may edit here, such as a mask shape without an editable mask.
    Forbidden,
}

/// The ring a brush cursor draws: the brush footprint as it appears on screen.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BrushOutline {
    diameter: u32,
    color: [u8; 3],
    erase: bool,
}

impl BrushOutline {
    /// Smallest ring a host draws, in physical pixels, so tiny brushes stay visible.
    pub const MIN_DIAMETER: u32 = 2;
    /// Largest ring a tool requests, in physical pixels; larger footprints fall back to the arrow
    /// so a native cursor never needs an unbounded bitmap.
    pub const MAX_DIAMETER: u32 = 2048;

    /// Rings below this diameter keep their exact pixel size.
    pub const EXACT_DIAMETER: u32 = 64;

    /// A ring about `diameter` physical pixels across, drawn in the target mask's sRGB `color`.
    /// `erase` adds the erase mark shown while Alt or a pen's eraser end turns the brush into an
    /// eraser. The diameter is clamped to the supported range; from [`Self::EXACT_DIAMETER`] up
    /// it snaps to 32 steps per doubling, within 1.6% of the request, so zooming a large brush
    /// reuses a bounded set of native cursors instead of one per camera scale.
    pub fn new(diameter: u32, color: [u8; 3], erase: bool) -> Self {
        let diameter = diameter.clamp(Self::MIN_DIAMETER, Self::MAX_DIAMETER);
        let diameter = if diameter < Self::EXACT_DIAMETER {
            diameter
        } else {
            let step = 1 << (diameter.ilog2() - 5);
            ((diameter + step / 2) / step * step).min(Self::MAX_DIAMETER)
        };
        Self { diameter, color, erase }
    }
    /// Ring diameter in physical pixels.
    pub const fn diameter(self) -> u32 {
        self.diameter
    }
    /// Ring colour as sRGB bytes.
    pub const fn color(self) -> [u8; 3] {
        self.color
    }
    /// Whether the ring carries the erase mark.
    pub const fn erase(self) -> bool {
        self.erase
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_outlines_clamp_to_a_visible_and_bounded_native_ring() {
        assert_eq!(BrushOutline::new(0, [1, 2, 3], false).diameter(), BrushOutline::MIN_DIAMETER);
        assert_eq!(BrushOutline::new(u32::MAX, [1, 2, 3], false).diameter(), 2048);
        let snapped: Vec<u32> = [63, 64, 65, 129, 130, 1000, 1017, 2047]
            .map(|diameter| BrushOutline::new(diameter, [0; 3], false).diameter())
            .to_vec();
        assert_eq!(snapped, [63, 64, 66, 128, 132, 1008, 1024, 2048]);
        let sizes: std::collections::BTreeSet<u32> = (0..=4096)
            .map(|diameter| BrushOutline::new(diameter, [0; 3], false).diameter())
            .collect();
        assert!(sizes.len() <= 62 + 6 * 32 + 1, "{} distinct rings", sizes.len());
        let outline = BrushOutline::new(37, [10, 20, 30], true);
        assert_eq!([outline.diameter(), u32::from(outline.erase())], [37, 1]);
        assert_eq!(outline.color(), [10, 20, 30]);
        assert_ne!(
            CursorIntent::BrushOutline(outline),
            CursorIntent::BrushOutline(BrushOutline::new(37, [10, 20, 30], false))
        );
    }
}
