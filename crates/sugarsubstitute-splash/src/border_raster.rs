// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Rasterize the console CSS border's independently painted, square-capped cubic arcs.

use crate::shapes::{AllocationError, append_cubic, byte_product, paint_outline};
use tiny_skia::{PathBuilder, Pixmap, Rect};

/// Paint one logical cubic using the reference cosmetic pen at one device pixel.
///
/// Larger device pens use an allocated outline mask and propagate its allocation cause.
pub(super) fn paint(
    target: &mut Pixmap,
    points: [[f64; 2]; 4],
    color: [u8; 4],
    scale: f32,
    clip: Rect,
) -> Result<(), AllocationError> {
    if scale == 1.0 {
        cosmetic(target, points, color, 6, (true, true), clip);
        return Ok(());
    }
    let normals =
        [normal(points[0], points[1]), normal(points[1], points[2]), normal(points[2], points[3])];
    let inner = offset(points, normals, 0.5);
    let outer = offset(points, normals, -0.5);
    let start_tangent = [normals[0][1], -normals[0][0]];
    let end_tangent = [normals[2][1], -normals[2][0]];
    let scale = f64::from(scale);
    let mut path = PathBuilder::new();
    let device =
        |point: [f64; 2]| point.map(|value| ((value * scale * 64.0).round() / 64.0) as f32);
    let start = device(inner[0]);
    path.move_to(start[0], start[1]);
    append_cubic(&mut path, inner, scale);
    for point in [add(inner[3], end_tangent, 0.5), add(outer[3], end_tangent, 0.5), outer[3]] {
        let point = device(point);
        path.line_to(point[0], point[1]);
    }
    append_cubic(&mut path, [outer[3], outer[2], outer[1], outer[0]], scale);
    for point in [add(outer[0], start_tangent, -0.5), add(inner[0], start_tangent, -0.5)] {
        let point = device(point);
        path.line_to(point[0], point[1]);
    }
    path.close();
    if let Some(path) = path.finish() {
        paint_outline(target, &path, color, clip)?;
    }
    Ok(())
}

/// Derive the unit normal of a nonzero cubic control edge.
fn normal(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    [-dy / length, dx / length]
}

/// Offset tangent intersections while retaining the cubic's endpoint normals.
fn offset(points: [[f64; 2]; 4], normals: [[f64; 2]; 3], distance: f64) -> [[f64; 2]; 4] {
    let join = |a: [f64; 2], b: [f64; 2]| {
        let denominator = 1.0 + a[0] * b[0] + a[1] * b[1];
        [(a[0] + b[0]) / denominator, (a[1] + b[1]) / denominator]
    };
    [
        add(points[0], normals[0], distance),
        add(points[1], join(normals[0], normals[1]), distance),
        add(points[2], join(normals[1], normals[2]), distance),
        add(points[3], normals[2], distance),
    ]
}

/// Translate a point along a unit tangent or normal.
fn add(point: [f64; 2], direction: [f64; 2], distance: f64) -> [f64; 2] {
    [point[0] + direction[0] * distance, point[1] + direction[1] * distance]
}

/// Subdivide the one-pixel pen's transformed cubic before fixed-point line integration.
fn cosmetic(
    target: &mut Pixmap,
    points: [[f64; 2]; 4],
    color: [u8; 4],
    depth: u8,
    caps: (bool, bool),
    clip: Rect,
) {
    let [a, b, c, d] = points;
    let chord = [a[0] - d[0], a[1] - d[1]];
    let tolerance = 0.25 * (chord[0].abs() + chord[1].abs());
    let cross =
        |point: [f64; 2]| (chord[0] * (d[1] - point[1]) - chord[1] * (d[0] - point[0])).abs();
    if depth == 0 || (cross(b) < tolerance && cross(c) < tolerance) {
        line(target, a, d, color, caps, clip);
        return;
    }
    let middle = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let ab = middle(a, b);
    let bc = middle(b, c);
    let cd = middle(c, d);
    let abc = middle(ab, bc);
    let bcd = middle(bc, cd);
    let split = middle(abc, bcd);
    cosmetic(target, [a, ab, abc, split], color, depth - 1, (caps.0, false), clip);
    cosmetic(target, [split, bcd, cd, d], color, depth - 1, (false, caps.1), clip);
}

/// Integrate one antialiased line on a 26.6 position and 16.16 slope grid.
fn line(
    target: &mut Pixmap,
    a: [f64; 2],
    b: [f64; 2],
    color: [u8; 4],
    caps: (bool, bool),
    clip: Rect,
) {
    let (mut start_cap, mut end_cap) = caps;
    let mut a = a.map(|value| (value * 64.0) as i64);
    let mut b = b.map(|value| (value * 64.0) as i64);
    let vertical = (b[0] - a[0]).abs() < (b[1] - a[1]).abs();
    if vertical {
        a.swap(0, 1);
        b.swap(0, 1);
    }
    if a[0] > b[0] {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut start_cap, &mut end_cap);
    }
    let length = b[0] - a[0];
    if length == 0 {
        return;
    }
    let slope = ((b[1] - a[1]) * 65536) / length;
    let mut transverse = (a[1] - 32) * 1024 - ((((a[0] & 63) - 32) * slope) >> 6);
    if start_cap {
        a[0] -= 32;
        transverse -= slope >> 1;
    }
    if end_cap {
        b[0] += 32;
    }
    let first = a[0] >> 6;
    let last = b[0] >> 6;
    for major in first..=last {
        let extent = if first == last {
            b[0] - a[0]
        } else if major == first {
            64 - (a[0] & 63)
        } else if major == last {
            b[0] & 63
        } else {
            64
        };
        let fraction = (transverse >> 8) & 255;
        for (minor, weight) in
            [(transverse >> 16, 255 - fraction), ((transverse >> 16) + 1, fraction)]
        {
            let (x, y) = if vertical { (minor, major) } else { (major, minor) };
            if x >= clip.left() as i64
                && x < clip.right() as i64
                && y >= clip.top() as i64
                && y < clip.bottom() as i64
            {
                pixel(target, x, y, color, ((weight * extent) >> 6) as u8);
            }
        }
        transverse += slope;
    }
}

/// Blend one pen sample without writing outside the native paint target.
pub(super) fn pixel(target: &mut Pixmap, x: i64, y: i64, color: [u8; 4], coverage: u8) {
    if x < 0 || y < 0 || x >= i64::from(target.width()) || y >= i64::from(target.height()) {
        return;
    }
    let index = (y as usize * target.width() as usize + x as usize) * 4;
    let alpha = byte_product(color[3], coverage);
    let source = [
        byte_product(byte_product(color[0], color[3]), coverage),
        byte_product(byte_product(color[1], color[3]), coverage),
        byte_product(byte_product(color[2], color[3]), coverage),
        alpha,
    ];
    for (destination, source) in target.data_mut()[index..index + 4].iter_mut().zip(source) {
        *destination = source + byte_product(*destination, 255 - alpha);
    }
}
