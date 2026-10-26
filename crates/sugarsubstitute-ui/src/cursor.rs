// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded native hand artwork with a stable logical hotspot and DPI-scaled outlines.
use std::io;

/// Gesture feedback represented by the shared native hand artwork.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandState {
    /// Available direct manipulation.
    Open,
    /// An active captured drag.
    Closed,
}
/// Fixed-capacity monochrome AND/XOR masks; no native handle or heap allocation is created.
pub struct HandCursor {
    width: u32,
    and: [u8; 2048],
    xor: [u8; 2048],
}
impl HandCursor {
    /// Rasterize shared hand artwork at a physical square size between 16 and 128 pixels.
    /// Rows use the native monochrome 16-bit alignment. Every pixel has explicit coverage.
    /// # Errors
    /// Rejects sizes outside the bounded native artwork contract.
    pub fn new(state: HandState, width: u32) -> io::Result<Self> {
        if !(16..=128).contains(&width) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid native hand size"));
        }
        let path: &[[f64; 2]] = match state {
            HandState::Open => &[
                [10., 29.],
                [8., 25.],
                [3., 19.],
                [2., 16.],
                [3., 14.],
                [5., 14.],
                [8., 17.],
                [8., 7.],
                [9., 5.],
                [11., 5.],
                [12., 7.],
                [12., 3.],
                [13., 1.],
                [15., 1.],
                [16., 3.],
                [16., 6.],
                [17., 3.],
                [19., 3.],
                [20., 5.],
                [20., 9.],
                [21., 6.],
                [23., 6.],
                [24., 8.],
                [24., 20.],
                [23., 24.],
                [21., 29.],
            ],
            HandState::Closed => &[
                [8., 28.],
                [5., 22.],
                [2., 19.],
                [2., 16.],
                [4., 14.],
                [7., 16.],
                [7., 12.],
                [8., 10.],
                [10., 10.],
                [11., 11.],
                [12., 9.],
                [14., 9.],
                [15., 10.],
                [16., 8.],
                [18., 8.],
                [19., 9.],
                [21., 10.],
                [23., 12.],
                [23., 21.],
                [21., 28.],
            ],
        };
        let mut result = Self { width, and: [255; 2048], xor: [0; 2048] };
        let stride = width.div_ceil(16) as usize * 2;
        for y in 0..width {
            for x in 0..width {
                let point = [
                    (f64::from(x) + 0.5) * 32. / f64::from(width),
                    (f64::from(y) + 0.5) * 32. / f64::from(width),
                ];
                let (inside, edge) = coverage(path, point);
                if inside || edge {
                    let index = y as usize * stride + x as usize / 8;
                    let bit = 1 << (7 - x % 8);
                    result.and[index] &= !bit;
                    if inside && !edge {
                        result.xor[index] |= bit;
                    }
                }
            }
        }
        Ok(result)
    }
    /// Physical square extent.
    pub const fn width(&self) -> u32 {
        self.width
    }
    /// Physical hotspot, preserving the same logical point across display scales and hand states.
    pub const fn hotspot(&self) -> [u32; 2] {
        [self.width * 12 / 32, self.width / 2]
    }
    /// Transparent background/opaque shape mask, with native word-aligned rows.
    pub fn and_bits(&self) -> &[u8] {
        &self.and[..self.byte_len()]
    }
    /// White fill/black outline mask, with the same aligned row layout as AND coverage.
    pub fn xor_bits(&self) -> &[u8] {
        &self.xor[..self.byte_len()]
    }
    fn byte_len(&self) -> usize {
        self.width.div_ceil(16) as usize * 2 * self.width as usize
    }
}
fn coverage(path: &[[f64; 2]], point: [f64; 2]) -> (bool, bool) {
    let mut inside = false;
    let mut edge = false;
    for index in 0..path.len() {
        let a = path[index];
        let b = path[(index + 1) % path.len()];
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        let delta = [b[0] - a[0], b[1] - a[1]];
        let projection = (((point[0] - a[0]) * delta[0] + (point[1] - a[1]) * delta[1])
            / (delta[0] * delta[0] + delta[1] * delta[1]))
            .clamp(0., 1.);
        let distance = (point[0] - a[0] - projection * delta[0]).powi(2)
            + (point[1] - a[1] - projection * delta[1]).powi(2);
        edge |= distance <= 0.75 * 0.75;
    }
    (inside, edge)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpi_sizes_preserve_aligned_masks_hotspots_and_distinct_gesture_feedback() -> io::Result<()> {
        for size in [16, 32, 40, 48, 64, 80, 96, 128] {
            let open = HandCursor::new(HandState::Open, size)?;
            let closed = HandCursor::new(HandState::Closed, size)?;
            assert_eq!(open.width(), size);
            assert_eq!(open.hotspot(), closed.hotspot());
            assert_ne!(open.and_bits(), closed.and_bits());
            assert_eq!(open.and_bits().len(), size.div_ceil(16) as usize * 2 * size as usize);
            assert!(open.and_bits().contains(&255));
            assert!(open.xor_bits().iter().any(|byte| *byte != 0));
            assert!(open.and_bits().iter().zip(open.xor_bits()).all(|(a, x)| a & x == 0));
        }
        assert!(HandCursor::new(HandState::Open, 0).is_err());
        assert!(HandCursor::new(HandState::Open, 129).is_err());
        Ok(())
    }
}
