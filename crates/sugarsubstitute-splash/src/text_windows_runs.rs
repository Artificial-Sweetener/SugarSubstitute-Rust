// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Copy DirectWrite's synchronous shaping callbacks into owned glyph runs.

use crate::text::memory::{self, MAX_GLYPHS, MAX_RUNS, METADATA_BYTES};
use std::{cell::RefCell, ffi::c_void};
use windows::{
    Win32::{Foundation::E_INVALIDARG, Graphics::DirectWrite::*},
    core::{BOOL, IUnknown, Ref, implement},
};

/// Retain one shaped run without borrowing a layout or its callback buffers.
pub(super) struct GlyphRun {
    /// Retained COM face providing native glyph coverage.
    pub face: IDWriteFontFace,
    /// Font size in logical pixels.
    pub size: f32,
    /// Baseline in logical pixels.
    pub baseline: (f32, f32),
    /// Native shaped glyph identities.
    pub indices: Vec<u16>,
    /// Native shaped horizontal advances.
    pub advances: Vec<f32>,
    /// Native shaped offsets relative to each advance.
    pub offsets: Vec<DWRITE_GLYPH_OFFSET>,
    /// Preserve direction established by the native shaper.
    pub level: u32,
    /// Foreground copied from this layout's retained private drawing effect.
    pub color: Option<[u8; 4]>,
}

/// Receive only synchronous callbacks from this owner's DirectWrite layout.
#[implement(IDWriteTextRenderer)]
pub(super) struct Collector {
    /// Owned results become available after the layout returns.
    pub runs: RefCell<Vec<GlyphRun>>,
    /// Preserve callback failures even when the layout returns a successful HRESULT.
    pub failure: RefCell<Option<std::io::Error>>,
    /// Exact effect references installed only by the current console layout owner.
    pub colors: Vec<(IUnknown, [u8; 4])>,
}

impl IDWritePixelSnapping_Impl for Collector_Impl {
    /// Leave logical shaping positions for the subsequent device transform.
    fn IsPixelSnappingDisabled(&self, _: *const c_void) -> windows::core::Result<BOOL> {
        Ok(true.into())
    }

    /// Fill the callback-owned matrix with the logical identity transform.
    fn GetCurrentTransform(
        &self,
        _: *const c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> windows::core::Result<()> {
        if transform.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: DirectWrite supplies writable storage for one matrix during Draw.
        unsafe { transform.write(DWRITE_MATRIX { m11: 1.0, m22: 1.0, ..Default::default() }) };
        Ok(())
    }

    /// Keep original fourteen-pixel label layout independent of capture scale.
    fn GetPixelsPerDip(&self, _: *const c_void) -> windows::core::Result<f32> {
        Ok(1.0)
    }
}

impl Collector_Impl {
    /// Copy synchronous DirectWrite arrays, retaining every failure outside its HRESULT.
    fn collect_glyph_run(
        &self,
        x: f32,
        y: f32,
        run: *const DWRITE_GLYPH_RUN,
        effect: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        if run.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: This private renderer is passed only to synchronous DirectWrite Draw.
        let run = unsafe { &*run };
        if run.glyphCount == 0 {
            return Ok(());
        }
        if run.glyphCount > MAX_GLYPHS as u32
            || run.glyphIndices.is_null()
            || run.fontFace.is_none()
            || !run.fontEmSize.is_finite()
            || run.fontEmSize <= 0.0
            || !x.is_finite()
            || !y.is_finite()
        {
            return Err(E_INVALIDARG.into());
        }
        let color = match effect.as_ref() {
            None => None,
            Some(effect) => Some(
                self.colors
                    .iter()
                    .find(|(identity, _)| identity == effect)
                    .ok_or(windows::core::Error::from(E_INVALIDARG))?
                    .1,
            ),
        };
        let count = run.glyphCount as usize;
        let face = run.fontFace.as_ref().ok_or(windows::core::Error::from(E_INVALIDARG))?.clone();
        let mut runs =
            self.runs.try_borrow_mut().map_err(|_| windows::core::Error::from(E_INVALIDARG))?;
        let total: usize = runs.iter().map(|run| run.indices.len()).sum();
        if runs.len() >= MAX_RUNS || total.saturating_add(count) > MAX_GLYPHS {
            return Err(E_INVALIDARG.into());
        }
        let allocation_error = |cause| {
            self.failure.replace(Some(cause));
            windows::core::Error::from(E_INVALIDARG)
        };
        let mut indices = memory::reserve(count, METADATA_BYTES / 8).map_err(allocation_error)?;
        let mut advances = memory::reserve(count, METADATA_BYTES / 8).map_err(allocation_error)?;
        let mut offsets = memory::reserve(count, METADATA_BYTES / 8).map_err(allocation_error)?;
        // SAFETY: DirectWrite owns these count-element arrays through the callback.
        unsafe {
            indices.extend_from_slice(std::slice::from_raw_parts(run.glyphIndices, count));
        }
        if run.glyphAdvances.is_null() {
            let mut font_metrics = DWRITE_FONT_METRICS::default();
            // SAFETY: The retained face writes into initialized local metrics storage.
            unsafe { face.GetMetrics(&mut font_metrics) };
            if font_metrics.designUnitsPerEm == 0 {
                return Err(E_INVALIDARG.into());
            }
            let mut metrics =
                memory::reserve(count, METADATA_BYTES / 4).map_err(allocation_error)?;
            metrics.resize(count, DWRITE_GLYPH_METRICS::default());
            // SAFETY: Both arrays contain exactly glyphCount elements and remain live.
            unsafe {
                face.GetDesignGlyphMetrics(
                    indices.as_ptr(),
                    run.glyphCount,
                    metrics.as_mut_ptr(),
                    run.isSideways.as_bool(),
                )?;
            }
            let scale = run.fontEmSize / f32::from(font_metrics.designUnitsPerEm);
            advances.extend(metrics.iter().map(|metric| {
                let advance = if run.isSideways.as_bool() {
                    metric.advanceHeight
                } else {
                    metric.advanceWidth
                };
                advance as f32 * scale
            }));
        } else {
            // SAFETY: DirectWrite supplies glyphCount advances when this optional array exists.
            unsafe {
                advances.extend_from_slice(std::slice::from_raw_parts(run.glyphAdvances, count))
            };
        }
        if run.glyphOffsets.is_null() {
            offsets.resize(count, DWRITE_GLYPH_OFFSET::default());
        } else {
            // SAFETY: DirectWrite supplies glyphCount offsets when this optional array exists.
            unsafe {
                offsets.extend_from_slice(std::slice::from_raw_parts(run.glyphOffsets, count))
            };
        }
        if advances.iter().any(|advance| !advance.is_finite())
            || offsets.iter().any(|offset| {
                !offset.advanceOffset.is_finite() || !offset.ascenderOffset.is_finite()
            })
        {
            return Err(E_INVALIDARG.into());
        }
        runs.push(GlyphRun {
            face,
            size: run.fontEmSize,
            baseline: (x, y),
            indices,
            advances,
            offsets,
            level: run.bidiLevel,
            color,
        });
        Ok(())
    }
}

impl IDWriteTextRenderer_Impl for Collector_Impl {
    /// Copy borrowed arrays and retain the font before returning to the layout.
    fn DrawGlyphRun(
        &self,
        _: *const c_void,
        x: f32,
        y: f32,
        _: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        _: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        effect: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        let result = self.collect_glyph_run(x, y, run, effect);
        if let Err(cause) = &result {
            let mut failure = self.failure.borrow_mut();
            if failure.is_none() {
                *failure = Some(std::io::Error::other(cause.clone()));
            }
        }
        result
    }

    /// Plain status labels never request underline geometry.
    fn DrawUnderline(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_UNDERLINE,
        _: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        Err(E_INVALIDARG.into())
    }

    /// Plain status labels never request strike-through geometry.
    fn DrawStrikethrough(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_STRIKETHROUGH,
        _: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        Err(E_INVALIDARG.into())
    }

    /// Plain status labels cannot embed external native drawing objects.
    fn DrawInlineObject(
        &self,
        _: *const c_void,
        _: f32,
        _: f32,
        _: Ref<IDWriteInlineObject>,
        _: BOOL,
        _: BOOL,
        _: Ref<IUnknown>,
    ) -> windows::core::Result<()> {
        Err(E_INVALIDARG.into())
    }
}
