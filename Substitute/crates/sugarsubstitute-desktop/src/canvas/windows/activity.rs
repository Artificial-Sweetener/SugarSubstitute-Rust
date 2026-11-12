// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit foreground pointer activity as native presentation demand independently of canvas bounds.
/// Foreground raw motion also arrives outside the client. Hidden/unfocused peers never request
/// a presentation; zero or invalid device samples are not motion. No coordinates are retained.
pub(super) fn pointer_motion(delta: (f64, f64), visible: bool, focused: bool) -> bool {
    visible
        && focused
        && delta.0.is_finite()
        && delta.1.is_finite()
        && (delta.0 != 0.0 || delta.1 != 0.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_motion_requests_presentation_without_requiring_a_client_position() {
        assert!(pointer_motion((2.0, -1.0), true, true));
        assert!(!pointer_motion((2.0, -1.0), false, true));
        assert!(!pointer_motion((2.0, -1.0), true, false));
        assert!(!pointer_motion((0.0, 0.0), true, true));
        assert!(!pointer_motion((f64::NAN, 1.0), true, true));
        assert!(!pointer_motion((1.0, f64::INFINITY), true, true));
    }
}
