// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the canvas editing cursors rendered at one window's DPI.
//!
//! Brush rings change size with the brush and the zoom, so the cache keeps only the most recently
//! used cursors. A cursor leaving the cache stays alive while a routed or selected resource still
//! holds it; the last holder destroys it.

use super::*;
use sugarsubstitute_ui::cursors::{CursorArt, PreciseMark};

/// Most rendered cursors one window keeps.
const CAPACITY: usize = 8;
/// Most rendered cursor pixel bytes one window keeps; the newest cursor always stays.
const BYTE_BUDGET: u64 = 32 << 20;

/// The artwork a custom canvas intent renders, or `None` for system and bundled cursors.
pub(super) fn art(intent: CursorIntent) -> Option<CursorArt> {
    Some(match intent {
        CursorIntent::Arrow
        | CursorIntent::OpenHand
        | CursorIntent::ClosedHand
        | CursorIntent::Resize(_)
        | CursorIntent::Forbidden => return None,
        CursorIntent::Affine(cursor) => {
            CursorArt::Affine { rotation: cursor.rotation(), degrees: cursor.degrees() }
        }
        CursorIntent::BrushOutline(outline) => CursorArt::BrushOutline {
            diameter: outline.diameter(),
            color: outline.color(),
            erase: outline.erase(),
        },
        CursorIntent::Precise => CursorArt::Precise(PreciseMark::None),
        CursorIntent::PreciseAdd => CursorArt::Precise(PreciseMark::Add),
        CursorIntent::PreciseSubtract => CursorArt::Precise(PreciseMark::Subtract),
        CursorIntent::SelectionTranslate | CursorIntent::Move => CursorArt::SelectionTranslate,
        CursorIntent::MoveCut => CursorArt::MoveCut,
    })
}

/// Recently rendered cursors keyed by artwork and DPI, most recent last.
#[derive(Default)]
pub(super) struct Rendered(Vec<Entry>);

struct Entry {
    art: CursorArt,
    dpi: u32,
    bytes: u64,
    cursor: Rc<OwnedCursor>,
}

impl Rendered {
    /// The cursor for `art` at `dpi`, rendering it on a miss and evicting the least recently used
    /// entry beyond capacity.
    pub(super) fn get(
        &mut self,
        art: CursorArt,
        dpi: u32,
    ) -> Result<Rc<OwnedCursor>, Box<dyn Error>> {
        if let Some(index) = self.0.iter().position(|entry| entry.art == art && entry.dpi == dpi) {
            let entry = self.0.remove(index);
            let cursor = entry.cursor.clone();
            self.0.push(entry);
            return Ok(cursor);
        }
        let image = art
            .render(dpi as f32 / 96.0)
            .map_err(|cause| DesktopError::new("canvas_cursor_render", cause))?;
        let cursor = Rc::new(resource::rendered_cursor(&image)?);
        let bytes = u64::from(image.width()) * u64::from(image.height()) * 4;
        self.0.push(Entry { art, dpi, bytes, cursor: cursor.clone() });
        while self.0.len() > CAPACITY
            || (self.0.len() > 1
                && self.0.iter().map(|entry| entry.bytes).sum::<u64>() > BYTE_BUDGET)
        {
            self.0.remove(0);
        }
        Ok(cursor)
    }

    /// Drop cursors rendered for any DPI other than `dpi`.
    pub(super) fn keep_dpi(&mut self, dpi: u32) {
        self.0.retain(|entry| entry.dpi == dpi);
    }

    /// Drop every rendered cursor.
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    /// Number of cached cursors.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    /// Pixel bytes of the cached cursors.
    #[cfg(test)]
    pub(super) fn bytes(&self) -> u64 {
        self.0.iter().map(|entry| entry.bytes).sum()
    }
}
