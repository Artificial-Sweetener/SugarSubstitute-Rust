// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check the caption's window buttons against their reference geometry at any client width.
use super::*;

const CAPTIONS: MainCaptionLabels = MainCaptionLabels {
    minimize: "Minimize",
    maximize: "Maximize",
    restore: "Restore",
    close: "Close",
};

#[test]
fn window_buttons_keep_their_reference_geometry() {
    for (width, maximized) in [(304.0, false), (1280.0, false), (1600.0, true)] {
        let controls = caption_controls(width, maximized, CAPTIONS);
        assert_eq!(controls.map(|control| control.id), [1, 2, 3]);
        for (index, control) in controls.iter().enumerate() {
            assert_eq!(control.bounds.x, width - 138.0 + index as f64 * 46.0);
            assert_eq!(control.bounds.width, 46.0);
            assert_eq!(control.bounds.y, 1.0);
            assert_eq!(control.bounds.height, 32.0);
            assert_eq!(control.close, index == 2);
            assert!(control.visible && control.enabled);
        }
        assert_eq!(controls[0].label, CAPTIONS.minimize);
        assert_eq!(controls[1].label, if maximized { CAPTIONS.restore } else { CAPTIONS.maximize });
        assert_eq!(controls[2].label, CAPTIONS.close);
    }
}

#[test]
fn the_buttons_fill_exactly_the_width_the_page_leaves_free() {
    let controls = caption_controls(1280.0, false, CAPTIONS);
    let span = controls[2].bounds.x + controls[2].bounds.width - controls[0].bounds.x;
    assert_eq!(span, f64::from(CAPTION_WIDTH));
    assert!(
        controls.iter().all(|control| {
            control.bounds.y + control.bounds.height <= f64::from(TITLEBAR_HEIGHT)
        })
    );
}
