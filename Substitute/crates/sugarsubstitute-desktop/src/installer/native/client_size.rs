// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep a form window's client at its logical size on whatever display scale the window has.
//!
//! The window's own scale is the authority: Windows may create a window at one monitor's scale
//! and render it at another, and a move between monitors may leave the window's pixel size
//! unchanged. On its first observation and on every scale change the window is given its logical
//! size at that scale, never larger than its monitor's work area holds; between scale changes a
//! size the user chose becomes the logical size kept from then on.

/// The logical client size a form window keeps across display scales.
#[derive(Debug)]
pub(super) struct ClientSize {
    logical: [f64; 2],
    scale: Option<f64>,
    /// The physical size last asked for at the current scale and not yet observed.
    requested: Option<[u32; 2]>,
    /// The physical size that matches `logical` at the current scale.
    current: [u32; 2],
}

impl ClientSize {
    /// Keep `logical` client pixels.
    pub(super) fn new(logical: [f64; 2]) -> Self {
        Self { logical, scale: None, requested: None, current: [0, 0] }
    }

    /// Observe the window at `actual` physical client pixels and `scale`, with `limit` the largest
    /// client its monitor's work area holds. Returns the physical client size to apply, or `None`
    /// when the window already has its size.
    pub(super) fn observe(
        &mut self,
        actual: [u32; 2],
        scale: f64,
        limit: [u32; 2],
    ) -> Option<[u32; 2]> {
        if self.scale == Some(scale) {
            match self.requested.take() {
                // The window kept another size than asked for, such as its minimum: that is
                // the size it has at this scale.
                Some(requested) if requested != actual => self.adopt(actual, scale),
                Some(_) => {}
                // The user resized the window.
                None if actual != self.current => self.adopt(actual, scale),
                None => {}
            }
            return None;
        }
        self.scale = Some(scale);
        let wanted = [0, 1].map(|axis| {
            let extent = (self.logical[axis] * scale).round().clamp(1.0, f64::from(u32::MAX));
            (extent as u32).min(limit[axis].max(1))
        });
        self.current = wanted;
        if wanted == actual {
            return None;
        }
        self.requested = Some(wanted);
        Some(wanted)
    }

    fn adopt(&mut self, actual: [u32; 2], scale: f64) {
        self.logical = actual.map(|extent| f64::from(extent) / scale);
        self.current = actual;
    }
}

#[cfg(test)]
mod tests {
    use super::ClientSize;

    const ROOMY: [u32; 2] = [8000, 8000];

    /// A window created at 100% sizes but rendering at 150% or 175% gets its scaled size once.
    #[test]
    fn a_window_created_at_another_scale_takes_its_logical_size() {
        for (scale, wanted) in [(1.0, None), (1.5, Some([1770, 1140])), (1.75, Some([2065, 1330]))]
        {
            let mut size = ClientSize::new([1180.0, 760.0]);
            assert_eq!(size.observe([1180, 760], scale, ROOMY), wanted, "scale {scale}");
            if let Some(applied) = wanted {
                assert_eq!(size.observe(applied, scale, ROOMY), None);
            }
            assert_eq!(size.observe(wanted.unwrap_or([1180, 760]), scale, ROOMY), None);
        }
    }

    /// A size the user chose is kept in logical pixels when the window changes monitor.
    #[test]
    fn a_user_size_survives_a_scale_change() {
        let mut size = ClientSize::new([1180.0, 760.0]);
        assert_eq!(size.observe([1180, 760], 1.0, ROOMY), None);
        assert_eq!(size.observe([900, 600], 1.0, ROOMY), None);
        assert_eq!(size.observe([900, 600], 1.75, ROOMY), Some([1575, 1050]));
        assert_eq!(size.observe([1575, 1050], 1.75, ROOMY), None);
        assert_eq!(size.observe([1575, 1050], 1.5, ROOMY), Some([1350, 900]));
    }

    /// The window never asks for more than its monitor's work area holds, and a window that keeps
    /// another size than asked for is not asked again at the same scale.
    #[test]
    fn the_work_area_bounds_the_size_and_a_refusal_is_accepted() {
        let mut size = ClientSize::new([1180.0, 760.0]);
        assert_eq!(size.observe([1180, 760], 1.5, [1366, 700]), Some([1366, 700]));
        assert_eq!(size.observe([1366, 720], 1.5, [1366, 700]), None);
        assert_eq!(size.observe([1366, 720], 1.5, [1366, 700]), None);
    }
}
