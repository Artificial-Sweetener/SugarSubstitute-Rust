// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Build the small rounded silhouettes shared by the console and loading bar.

use tiny_skia::{Path, PathBuilder};

/// Construct a rounded rectangle using the same cubic circle approximation as Qt.
pub(crate) fn rounded(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<Path> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let radius = radius.min(width / 2.0).min(height / 2.0);
    let curve = radius * 0.552_284_8;
    let right = x + width;
    let bottom = y + height;
    let mut path = PathBuilder::new();
    path.move_to(x + radius, y);
    path.line_to(right - radius, y);
    path.cubic_to(right - radius + curve, y, right, y + radius - curve, right, y + radius);
    path.line_to(right, bottom - radius);
    path.cubic_to(
        right,
        bottom - radius + curve,
        right - radius + curve,
        bottom,
        right - radius,
        bottom,
    );
    path.line_to(x + radius, bottom);
    path.cubic_to(x + radius - curve, bottom, x, bottom - radius + curve, x, bottom - radius);
    path.line_to(x, y + radius);
    path.cubic_to(x, y + radius - curve, x + radius - curve, y, x + radius, y);
    path.close();
    path.finish()
}
