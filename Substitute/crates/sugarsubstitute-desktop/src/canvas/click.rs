// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recognize native double clicks for windows whose class does not request them.
//!
//! Neither the standalone canvas peer nor Tao's main window registers `CS_DBLCLKS`, so both
//! pair primary presses themselves with the user's Windows double-click time and distance.

/// One primary press: its wrapping 32-bit message time and physical client position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Click {
    /// `GetMessageTime` of the press, in milliseconds; it wraps about every 49.7 days.
    pub(crate) time: u32,
    /// Physical client position of the press.
    pub(crate) point: [i32; 2],
}

/// Match physical double-click bounds and the native wrapping 32-bit message timestamp.
///
/// `bounds` is the `SM_CXDOUBLECLK`/`SM_CYDOUBLECLK` rectangle for the window's DPI and
/// `limit` is `GetDoubleClickTime`; nonpositive bounds never match.
pub(crate) fn double_click(previous: Click, current: Click, bounds: [i32; 2], limit: u32) -> bool {
    bounds.iter().all(|bound| *bound > 0)
        && current.time.wrapping_sub(previous.time) <= limit
        && (0..2).all(|axis| {
            (i64::from(current.point[axis]) - i64::from(previous.point[axis])).abs() * 2
                <= i64::from(bounds[axis])
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_click_matches_native_time_and_physical_distance_including_timestamp_wrap() {
        let previous = Click { time: u32::MAX - 50, point: [20, -30] };
        assert!(double_click(previous, Click { time: 25, point: [22, -28] }, [4, 4], 100));
        assert!(!double_click(previous, Click { time: 25, point: [23, -28] }, [4, 4], 100));
        assert!(!double_click(previous, Click { time: 50, point: [20, -30] }, [4, 4], 100));
        assert!(!double_click(previous, Click { time: 25, point: [20, -30] }, [0, 0], 100));
    }
}
