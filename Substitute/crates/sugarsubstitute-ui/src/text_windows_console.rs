// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shape owned console styles and retain native hit-test custody through bounded painting.

use super::{FontOwner, GlyphPaint, runs::Collector};
use crate::bitmap::Bitmap;
use crate::text::{
    console::{
        Background, BackgroundPaint, ConsoleFont, ConsoleGlyph, ConsolePaintSpec, ConsoleRowView,
    },
    memory::{self, Allocation, MAX_GLYPHS, MAX_RUNS, METADATA_BYTES},
};
use std::{cell::RefCell, ffi::c_void, io, num::NonZeroUsize};
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_contracts::presentation::console_color;
use windows::{
    Win32::{Foundation::E_INVALIDARG, Graphics::DirectWrite::*},
    core::{BOOL, ComObject, IUnknown, Interface, implement},
};

/// This private COM identity carries no drawing operation or native privilege.
#[implement(IDWritePixelSnapping)]
struct ColorEffect;
impl IDWritePixelSnapping_Impl for ColorEffect_Impl {
    fn IsPixelSnappingDisabled(&self, _: *const c_void) -> windows::core::Result<BOOL> {
        Ok(true.into())
    }
    fn GetCurrentTransform(
        &self,
        _: *const c_void,
        value: *mut DWRITE_MATRIX,
    ) -> windows::core::Result<()> {
        if value.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: The caller supplies one writable ABI matrix for this synchronous callback.
        unsafe { value.write(DWRITE_MATRIX { m11: 1.0, m22: 1.0, ..Default::default() }) };
        Ok(())
    }
    fn GetPixelsPerDip(&self, _: *const c_void) -> windows::core::Result<f32> {
        Ok(1.0)
    }
}

impl ConsoleFont for FontOwner {
    type Paint = GlyphPaint;
    fn prepare_console(
        &mut self,
        row: ConsoleRowView<'_>,
        spec: ConsolePaintSpec,
        lease: Option<MemoryLease>,
    ) -> io::Result<GlyphPaint> {
        spec.validate()?;
        memory::validate(row.text, spec.surface.scale, spec.surface.width, spec.surface.height)?;
        if row.runs.len() > MAX_RUNS
            || row.runs.iter().any(|run| {
                run.start >= run.end || row.text.get(run.start as usize..run.end as usize).is_none()
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid projected console ranges",
            ));
        }
        let allocation = Allocation(lease);
        let metadata = allocation.claim(METADATA_BYTES)?;
        let mut text = memory::reserve::<u16>(MAX_GLYPHS, METADATA_BYTES / 8)?;
        text.extend(row.text.encode_utf16());
        // SAFETY: Text and format remain valid during layout construction, which copies them.
        let layout = unsafe {
            self.factory.CreateGdiCompatibleTextLayout(
                &text,
                &self.format,
                spec.extent[0],
                spec.extent[1],
                1.0,
                None,
                false,
            )?
        };
        // SAFETY: This owned layout has a single bounded row; clipping also applies to every glyph.
        unsafe {
            layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            if spec.font_size != 14.0 {
                layout.SetFontSize(
                    spec.font_size,
                    DWRITE_TEXT_RANGE { startPosition: 0, length: text.len() as u32 },
                )?;
            }
        }
        let mut colors = memory::reserve::<(IUnknown, [u8; 4])>(MAX_RUNS, METADATA_BYTES / 8)?;
        let mut ranges =
            memory::reserve::<(DWRITE_TEXT_RANGE, [u8; 4])>(MAX_RUNS, METADATA_BYTES / 8)?;
        for run in row.runs {
            if run.utf16_start >= run.utf16_end || run.utf16_end as usize > text.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid projected console range",
                ));
            }
            let range = DWRITE_TEXT_RANGE {
                startPosition: run.utf16_start,
                length: run.utf16_end - run.utf16_start,
            };
            let effect: IDWritePixelSnapping = ComObject::new(ColorEffect).to_interface();
            let identity = effect.cast::<IUnknown>()?;
            let foreground = console_color(run.style.foreground, spec.surface.theme, false)
                .ok_or_else(|| io::Error::other("Console foreground absent"))?;
            // SAFETY: The layout retains each effect; exact identities are retained by its callback owner.
            unsafe {
                layout.SetFontWeight(
                    if run.style.bold {
                        DWRITE_FONT_WEIGHT_BOLD
                    } else {
                        DWRITE_FONT_WEIGHT_NORMAL
                    },
                    range,
                )?;
                layout.SetDrawingEffect(&identity, range)?;
            }
            colors.push((identity, foreground));
            if let Some(background) = console_color(run.style.background, spec.surface.theme, true)
            {
                ranges.push((range, background));
            }
        }
        let collector = ComObject::new(Collector {
            runs: RefCell::new(memory::reserve(MAX_RUNS, METADATA_BYTES / 2)?),
            failure: RefCell::new(None),
            colors,
        });
        let renderer: IDWriteTextRenderer = collector.to_interface();
        // SAFETY: Draw is synchronous; the private collector copies bounded buffers and retains each face.
        let result = unsafe { layout.Draw(None, &renderer, 0.0, 0.0) };
        if let Some(cause) = collector.get().failure.replace(None) {
            return Err(cause);
        }
        result?;
        let runs = collector.get().runs.replace(Vec::new());
        let background = ConsoleBackground::new(layout, ranges, spec)?;
        let mut paint =
            GlyphPaint::new(self.factory.clone(), runs, metadata, allocation, spec.label());
        paint.bind_console(spec, background);
        Ok(paint)
    }
}
impl ConsoleGlyph for GlyphPaint {
    fn step(&mut self, target: &mut Bitmap, maximum: NonZeroUsize) -> io::Result<bool> {
        GlyphPaint::step(self, target, maximum)
    }
}

/// Keep the actual layout until one hit-test range per turn has supplied all background geometry.
pub(super) struct ConsoleBackground {
    layout: Option<IDWriteTextLayout>,
    ranges: Vec<(DWRITE_TEXT_RANGE, [u8; 4])>,
    range: usize,
    metrics: Vec<DWRITE_HIT_TEST_METRICS>,
    spans: Vec<Background>,
    paint: Option<BackgroundPaint>,
    spec: ConsolePaintSpec,
}
impl ConsoleBackground {
    fn new(
        layout: IDWriteTextLayout,
        ranges: Vec<(DWRITE_TEXT_RANGE, [u8; 4])>,
        spec: ConsolePaintSpec,
    ) -> io::Result<Self> {
        let mut metrics = memory::reserve(MAX_GLYPHS, METADATA_BYTES / 4)?;
        metrics.resize(MAX_GLYPHS, DWRITE_HIT_TEST_METRICS::default());
        Ok(Self {
            layout: Some(layout),
            ranges,
            range: 0,
            metrics,
            spans: memory::reserve(MAX_GLYPHS, METADATA_BYTES / 4)?,
            paint: None,
            spec,
        })
    }
    pub(super) fn step(&mut self, target: &mut Bitmap, maximum: NonZeroUsize) -> io::Result<bool> {
        if let Some(paint) = &mut self.paint {
            return paint.step(target, maximum);
        }
        if self.range == self.ranges.len() {
            self.layout = None;
            self.metrics = Vec::new();
            self.ranges = Vec::new();
            self.paint = Some(BackgroundPaint::new(std::mem::take(&mut self.spans), self.spec)?);
            return Ok(false);
        }
        let (range, color) = self.ranges[self.range];
        let mut count = 0;
        let layout =
            self.layout.as_ref().ok_or_else(|| io::Error::other("Console layout absent"))?;
        // SAFETY: The retained layout writes only this initialized bounded metrics slice and count.
        // Insufficient native capacity is an observable error; geometry is never guessed or truncated.
        unsafe {
            layout.HitTestTextRange(
                range.startPosition,
                range.length,
                self.spec.origin[0],
                self.spec.top(),
                Some(&mut self.metrics),
                &mut count,
            )?;
        }
        if count as usize > self.metrics.len() || self.spans.len() + count as usize > MAX_GLYPHS {
            return Err(io::Error::other("Console hit-test geometry bound exceeded"));
        }
        for metric in &self.metrics[..count as usize] {
            self.spans.push(Background {
                bounds: [
                    metric.left,
                    metric.top,
                    metric.left + metric.width,
                    metric.top + metric.height,
                ],
                color,
            });
        }
        self.range += 1;
        Ok(false)
    }
}
