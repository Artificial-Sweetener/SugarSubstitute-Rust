// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independent full-viewport CPU reference for cropped device-only material qualification.
use super::*;

pub(super) struct Reference<'a> {
    pub primary: &'a [u32],
    pub primary_extent: Extent,
    pub columns: &'a [DisplaySample],
    pub rows: &'a [DisplaySample],
    pub fallback: &'a [u32],
    pub fallback_extent: Extent,
    pub fallback_columns: &'a [DisplaySample],
    pub fallback_rows: &'a [DisplaySample],
    pub overlay: &'a [OverlayPixel],
    pub matte: u32,
}
impl Reference<'_> {
    pub(super) fn encoded(&self, x: u32, y: u32) -> u32 {
        if let Some(p) = self.overlay.iter().rev().find(|p| p.x == x && p.y == y) {
            return u32::from_be_bytes([255, p.color[0], p.color[1], p.color[2]]);
        }
        sample(self.primary, self.primary_extent, self.columns[x as usize], self.rows[y as usize])
            .or_else(|| {
                sample(
                    self.fallback,
                    self.fallback_extent,
                    self.fallback_columns[x as usize],
                    self.fallback_rows[y as usize],
                )
            })
            .unwrap_or(self.matte)
    }
}
fn sample(pixels: &[u32], extent: Extent, x: DisplaySample, y: DisplaySample) -> Option<u32> {
    if !x.is_covered() || !y.is_covered() {
        return None;
    }
    let at = |x: u32, y: u32| pixels[(y * extent.width() + x) as usize];
    Some(bilinear_display_pixel(
        [
            at(x.first(), y.first()),
            at(x.second(), y.first()),
            at(x.first(), y.second()),
            at(x.second(), y.second()),
        ],
        x,
        y,
    ))
}
pub(super) struct Source<'a> {
    pub asset: AssetId,
    pub reference: &'a Reference<'a>,
}
impl RasterSource for Source<'_> {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> std::result::Result<(), ImageError> {
        if asset != self.asset || output.len() as u64 != region.pixels() {
            return Err(ImageError::Invalid("Foreign qualification raster request"));
        }
        for y in 0..region.height() {
            if context.cancellation_requested() {
                return Err(ImageError::Invalid("Qualification was cancelled"));
            }
            for x in 0..region.width() {
                output[(y * region.width() + x) as usize] =
                    SrgbDecoder::decode(self.reference.encoded(region.x() + x, region.y() + y))?;
            }
        }
        Ok(())
    }
}
pub(super) fn graph(
    asset: AssetId,
    extent: Extent,
    memory: &MemoryLease,
) -> Result<(Graph, NodeId)> {
    let mut graph = Graph::new(5, memory)?;
    let mut node = graph.add(Operation::Raster { asset, extent })?;
    let mut low = extent;
    for _ in 0..2 {
        low = Extent::new(low.width().div_ceil(2), low.height().div_ceil(2))?;
        node = graph.add(Operation::Resample {
            input: node,
            extent: low,
            sampling: Sampling::new([2.0; 2], [0.0; 2])?,
        })?;
    }
    node = graph.add(Operation::GaussianBlur { input: node, blur: GaussianBlur::new(2.5)? })?;
    node = graph.add(Operation::Resample {
        input: node,
        extent,
        sampling: Sampling::new([0.25; 2], [0.0; 2])?,
    })?;
    Ok((graph, node))
}
pub(super) fn coverage(bounds: [f64; 4], radius: f64, x: u32, y: u32) -> u8 {
    let [left, top, width, height] = bounds;
    let right = left + width;
    let bottom = top + height;
    let mut count = 0;
    for sy in 0..4 {
        for sx in 0..4 {
            let px = f64::from(x) + (f64::from(sx) + 0.5) / 4.0;
            let py = f64::from(y) + (f64::from(sy) + 0.5) / 4.0;
            if px < left || px >= right || py < top || py >= bottom {
                continue;
            }
            let cx = px.clamp(left + radius, right - radius);
            let cy = py.clamp(top + radius, bottom - radius);
            if (px - cx).powi(2) + (py - cy).powi(2) <= radius * radius {
                count += 1;
            }
        }
    }
    count
}
pub(super) fn mixed(original: u32, filtered: u32, count: u8) -> u32 {
    let a = original.to_be_bytes();
    let b = filtered.to_be_bytes();
    let c = f32::from(count) / 16.0;
    u32::from_be_bytes(std::array::from_fn(|i| {
        (f32::from(a[i]) * (1.0 - c) + f32::from(b[i]) * c).round() as u8
    }))
}
pub(super) fn pixels(extent: Extent, phase: u32) -> Vec<u32> {
    (0..extent.pixels())
        .map(|i| {
            let x = i as u32 % extent.width();
            let y = i as u32 / extent.width();
            let alpha = if (x / 37 + y / 29).is_multiple_of(11) {
                0
            } else if (x / 53 + y / 47).is_multiple_of(5) {
                128
            } else {
                255
            };
            let channel = |v: u32| ((v % 256) * alpha + 127) / 255;
            (alpha << 24)
                | (channel(x * 13 + y * 7 + phase) << 16)
                | (channel(x * 3 + y * 17 + phase * 3) << 8)
                | channel(x * 11 + y * 5 + phase * 7)
        })
        .collect()
}
