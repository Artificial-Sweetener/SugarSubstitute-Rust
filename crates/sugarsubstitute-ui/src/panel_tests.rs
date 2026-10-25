// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise shared panel placement and retained target ownership.

use super::{OutputPanel, PanelCursor};
use crate::bitmap::Bitmap;
use std::error::Error;

fn panel() -> OutputPanel {
    OutputPanel { x: 8.0, y: 12.0, width: 64.0, height: 36.0, dark: true }
}

#[test]
fn panel_translation_preserves_each_pixel_and_clip() -> Result<(), Box<dyn Error>> {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut original = Bitmap::new(192, 144).ok_or("Original panel absent")?;
        let mut moved = Bitmap::new(192, 144).ok_or("Moved panel absent")?;
        for (target, x, y) in [(&mut original, 8.0, 12.0), (&mut moved, 24.0, 20.0)] {
            let mut cursor = PanelCursor::new(192, 144, OutputPanel { x, y, ..panel() }, scale)?;
            while !cursor.step(target)? {}
        }
        let dx = (16.0 * scale) as u32;
        let dy = (8.0 * scale) as u32;
        for y in 0..144 {
            for x in 0..192 {
                let expected = if x >= dx && y >= dy {
                    original.pixel(x - dx, y - dy).ok_or("Original pixel absent")?
                } else {
                    tiny_skia::PremultipliedColorU8::TRANSPARENT
                };
                assert_eq!(moved.pixel(x, y), Some(expected), "{scale}: ({x}, {y})");
            }
        }
    }
    Ok(())
}

#[test]
fn invalid_panel_or_changed_target_cannot_publish_pixels() -> Result<(), Box<dyn Error>> {
    for value in [0.0, -1.0, f32::NAN, f32::INFINITY, 4097.0] {
        assert!(PanelCursor::new(100, 100, OutputPanel { width: value, ..panel() }, 1.0).is_err());
    }
    let mut cursor = PanelCursor::new(100, 100, panel(), 1.0)?;
    let mut wrong = Bitmap::new(100, 101).ok_or("Wrong target absent")?;
    assert!(cursor.step(&mut wrong).is_err());
    assert!(wrong.data().iter().all(|byte| *byte == 0));
    let mut right = Bitmap::new(100, 100).ok_or("Right target absent")?;
    while !cursor.step(&mut right)? {}
    assert!(right.data().iter().any(|byte| *byte != 0));
    Ok(())
}
