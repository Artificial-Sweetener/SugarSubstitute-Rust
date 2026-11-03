// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render source captures of an input document on an admitted worker.
//!
//! Image layers compose over transparency into straight sRGB `rgba8`; a mask layer captures in
//! the coverage image encoding (red = green = blue = coverage, alpha = 255 − coverage); the
//! selection captures as one coverage byte per pixel. A capture smaller than the document is an
//! exact area average of the document's pixels in linear premultiplied light (images) or
//! coverage (masks and selection), so every output pixel weighs exactly the document area it
//! covers. Scratch storage is charged to the job.
use super::*;
use sugarsubstitute_canvas::{CaptureLayers, MAX_SURFACE_EDGE};
use sugarsubstitute_image_ops::{Coverage, Evaluator, FULL_COVERAGE, SrgbEncoder, coverage_to_u8};

/// Retained evaluation tiles of one capture render.
const CAPTURE_TILES: usize = 64;

/// What one captured raster reads from its document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapturePixels {
    /// Image layers composed over transparency, as straight sRGB `rgba8`.
    Image(CaptureLayers),
    /// One mask layer in the coverage image encoding, `rgba8`.
    Mask(LayerId),
    /// The selection, one coverage byte per pixel; no selection is uncovered.
    Selection,
}

impl CapturePixels {
    /// Bytes each output pixel occupies.
    pub(crate) const fn bytes_per_pixel(self) -> u64 {
        match self {
            Self::Image(_) | Self::Mask(_) => 4,
            Self::Selection => 1,
        }
    }
}

/// Render `pixels` of `snapshot` at exactly `extent`, which is never larger than the document.
pub(in crate::native_canvas) fn render(
    snapshot: &DocumentSnapshot,
    sources: &[Source],
    pixels: CapturePixels,
    extent: Extent,
    context: &JobContext,
) -> Result<Vec<u8>, WorkError> {
    let document = snapshot.extent();
    if extent.width() > document.width() || extent.height() > document.height() {
        return Err(WorkError::invalid("A capture never enlarges its document"));
    }
    let bytes = extent.pixels() * pixels.bytes_per_pixel();
    let length =
        usize::try_from(bytes).map_err(|_| WorkError::invalid("Capture exceeds memory"))?;
    let _output = context.memory().claim(bytes)?;
    let mut output = Vec::new();
    output.try_reserve_exact(length).map_err(ImageError::from)?;
    output.resize(length, 0);
    match pixels {
        CapturePixels::Image(layers) => {
            image(snapshot, sources, layers, extent, &mut output, context)?
        }
        CapturePixels::Mask(layer) => {
            let coverage = snapshot
                .layers()
                .iter()
                .find(|candidate| candidate.id() == layer)
                .and_then(|layer| match layer.content() {
                    LayerContent::Coverage { coverage, .. } => Some(coverage),
                    _ => None,
                })
                .ok_or_else(|| WorkError::invalid("The captured mask layer is unavailable"))?;
            coverage_raster(coverage, extent, &mut output, context, |value, pixel| {
                pixel.copy_from_slice(&[value, value, value, 255 - value]);
            })?;
        }
        CapturePixels::Selection => match snapshot.selection() {
            Some(selection) => {
                coverage_raster(selection, extent, &mut output, context, |value, pixel| {
                    pixel[0] = value;
                })?;
            }
            None => output.fill(0),
        },
    }
    Ok(output)
}

/// Compose image layers at the finest grid a viewport can hold, then area-average to `extent`.
fn image(
    snapshot: &DocumentSnapshot,
    sources: &[Source],
    layers: CaptureLayers,
    extent: Extent,
    output: &mut [u8],
    context: &JobContext,
) -> Result<(), WorkError> {
    let document = snapshot.extent();
    let longest = document.width().max(document.height());
    let zoom = if longest > MAX_SURFACE_EDGE {
        f64::from(MAX_SURFACE_EDGE) / f64::from(longest)
    } else {
        1.0
    };
    let side = |value: u32| -> Result<u32, WorkError> {
        u32::try_from(
            ((f64::from(value) * zoom).ceil() as u64).clamp(1, u64::from(MAX_SURFACE_EDGE)),
        )
        .map_err(|_| WorkError::invalid("Capture surface overflow"))
    };
    let surface = Extent::new(side(document.width())?, side(document.height())?)?;
    let mut view = Viewport::new(surface)?;
    view.reframe([0.0, 0.0], zoom)?;
    let reconstruction = Reconstruction::finished_for_display_zoom(zoom)?;
    let compiled = snapshot.compile_capture(view, layers, reconstruction, context.memory())?;
    let (graph, node) = compiled.into_parts();
    let port = SourcePort(sources.to_vec());
    let mut evaluator = Evaluator::new(graph, port, CAPTURE_TILES, context.memory())?;
    let encoder = SrgbEncoder::new(context.memory())?;
    let mut area = Area::new(surface, extent, 4, context)?;
    for part in surface.region().tiles() {
        let tile = evaluator.region(node, part, context)?;
        let region = tile.region();
        for (index, pixel) in tile.pixels().iter().enumerate() {
            let x = region.x() + (index as u32 % region.width());
            let y = region.y() + (index as u32 / region.width());
            let channels = pixel.channels();
            match &mut area {
                Area::Identity { width } => {
                    let start = (y as usize * *width + x as usize) * 4;
                    output[start..start + 4].copy_from_slice(&encoder.encode_straight(channels));
                }
                Area::Average(average) => average.add(x, y, &channels.map(f64::from)),
            }
        }
    }
    if let Area::Average(average) = area {
        for (pixel, sums) in output.chunks_exact_mut(4).zip(average.sums.chunks_exact(4)) {
            let channels = [sums[0], sums[1], sums[2], sums[3]].map(|sum| sum as f32);
            pixel.copy_from_slice(&encoder.encode_straight(channels));
        }
    }
    Ok(())
}

/// Encode coverage at `extent` through `write`, which receives one 8-bit value per pixel.
fn coverage_raster(
    coverage: &Coverage,
    extent: Extent,
    output: &mut [u8],
    context: &JobContext,
    write: impl Fn(u8, &mut [u8]),
) -> Result<(), WorkError> {
    let bytes = output.len() / extent.pixels() as usize;
    let source = coverage.extent();
    let mut area = Area::new(source, extent, 1, context)?;
    let mut values = Vec::new();
    for part in source.region().tiles() {
        if context.cancellation_requested() {
            return Err(ImageError::Cancelled.into());
        }
        values.clear();
        values.resize(part.pixels() as usize, 0_u16);
        coverage.read(part, &mut values)?;
        for (index, value) in values.iter().enumerate() {
            let x = part.x() + (index as u32 % part.width());
            let y = part.y() + (index as u32 / part.width());
            match &mut area {
                Area::Identity { width } => {
                    let start = (y as usize * *width + x as usize) * bytes;
                    write(coverage_to_u8(*value), &mut output[start..start + bytes]);
                }
                Area::Average(average) => {
                    average.add(x, y, &[f64::from(*value) / f64::from(FULL_COVERAGE)]);
                }
            }
        }
    }
    if let Area::Average(average) = area {
        for (pixel, sum) in output.chunks_exact_mut(bytes).zip(&average.sums) {
            write((sum.clamp(0.0, 1.0) * 255.0).round() as u8, pixel);
        }
    }
    Ok(())
}

/// How source pixels reach the output: one to one, or averaged over exact areas.
enum Area {
    Identity { width: usize },
    Average(Average),
}

impl Area {
    fn new(
        source: Extent,
        target: Extent,
        channels: usize,
        context: &JobContext,
    ) -> Result<Self, WorkError> {
        if source == target {
            return Ok(Self::Identity { width: target.width() as usize });
        }
        Ok(Self::Average(Average::new(source, target, channels, context)?))
    }
}

/// Area-weighted sums of a reduction, each output pixel weighing the source area it covers.
struct Average {
    columns: Vec<[(u32, f64); 2]>,
    rows: Vec<[(u32, f64); 2]>,
    width: usize,
    channels: usize,
    sums: Vec<f64>,
    _memory: MemoryClaim,
}

impl Average {
    fn new(
        source: Extent,
        target: Extent,
        channels: usize,
        context: &JobContext,
    ) -> Result<Self, WorkError> {
        let sums = target.pixels() as usize * channels;
        let spans = (source.width() as usize + source.height() as usize) * 32;
        let memory = context.memory().claim((sums * 8 + spans) as u64)?;
        Ok(Self {
            columns: spans_of(source.width(), target.width()),
            rows: spans_of(source.height(), target.height()),
            width: target.width() as usize,
            channels,
            sums: vec![0.0; sums],
            _memory: memory,
        })
    }

    /// Add source pixel `(x, y)` to the output pixels it overlaps.
    fn add(&mut self, x: u32, y: u32, values: &[f64]) {
        for (row, row_weight) in self.rows[y as usize] {
            for (column, column_weight) in self.columns[x as usize] {
                let weight = row_weight * column_weight;
                if weight == 0.0 {
                    continue;
                }
                let start = (row as usize * self.width + column as usize) * self.channels;
                for (sum, value) in self.sums[start..start + self.channels].iter_mut().zip(values) {
                    *sum += value * weight;
                }
            }
        }
    }
}

/// The output pixels each source pixel of a `source`-long axis overlaps when it is reduced to
/// `target` pixels, with the overlap measured in output pixels; a reduction never spans more
/// than two output pixels per source pixel.
fn spans_of(source: u32, target: u32) -> Vec<[(u32, f64); 2]> {
    let scale = f64::from(target) / f64::from(source);
    (0..source)
        .map(|index| {
            let start = f64::from(index) * scale;
            let end = f64::from(index + 1) * scale;
            let first = (start.floor() as u32).min(target - 1);
            let boundary = f64::from(first + 1);
            if end <= boundary || first + 1 >= target {
                [(first, end.min(boundary) - start), (first, 0.0)]
            } else {
                [(first, boundary - start), (first + 1, end - boundary)]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_output_pixel_weighs_exactly_the_area_it_covers() {
        for (source, target) in [(10, 3), (7, 7), (1000, 1), (5, 4), (3, 2)] {
            let spans = spans_of(source, target);
            let mut totals = vec![0.0_f64; target as usize];
            for span in spans {
                for (pixel, weight) in span {
                    assert!(weight >= 0.0);
                    totals[pixel as usize] += weight;
                }
            }
            for total in totals {
                assert!((total - 1.0).abs() < 1e-9, "{source}->{target}: {total}");
            }
        }
    }
}
