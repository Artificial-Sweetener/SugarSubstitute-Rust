// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint transparent Windows 11 scrollbar chrome over the authoritative native scroll range.
use crate::DesktopError;
use windows::{
    Win32::{
        Foundation::{POINT, RECT},
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
        },
    },
    core::w,
};

/// Native interaction region; the engine owns capture, timers and the resulting scroll operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Part {
    None,
    Decrement,
    Increment,
    Thumb,
    PageBefore,
    PageAfter,
}

/// Physical client bounds and native position facts; `dark` is the Windows style color scheme.
/// `maximum` is the last admissible position; `hover` is the parent's 100-ms animation, 0..255.
#[derive(Clone, Copy)]
pub(super) struct Model {
    pub(super) bounds: RECT,
    pub(super) scale: f64,
    pub(super) minimum: i32,
    pub(super) maximum: i32,
    pub(super) page: u32,
    pub(super) position: i32,
    pub(super) dark: bool,
    pub(super) enabled: bool,
    pub(super) hover: u8,
    pub(super) pressed: Part,
}

/// Full-width hit regions remain stable while only the painted thumb expands.
#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub(super) track: RECT,
    pub(super) decrement: RECT,
    pub(super) increment: RECT,
    pub(super) thumb: RECT,
    pub(super) paint_thumb: D2D_RECT_F,
}

/// Resolve bounded native position facts into physical hit and publication geometry.
pub(super) fn geometry(model: &Model) -> Result<Geometry, DesktopError> {
    let bounds = model.bounds;
    let height = i64::from(bounds.bottom) - i64::from(bounds.top);
    let width = i64::from(bounds.right) - i64::from(bounds.left);
    if !model.scale.is_finite()
        || model.scale <= 0.0
        || model.scale * 18.0 > f64::from(i32::MAX)
        || height < 0
        || width < 0
        || height > i64::from(i32::MAX)
        || model.maximum < model.minimum
        || !(model.minimum..=model.maximum).contains(&model.position)
    {
        return Err(DesktopError::message(
            "console_scroll_geometry",
            "invalid native scroll geometry",
        ));
    }
    let extent = (12.0 * model.scale).round() as i32;
    let width = width.min(i64::from(extent)) as i32;
    let left = bounds.right.saturating_sub(width);
    let button = extent.min(height as i32 / 2);
    let start = bounds.top.saturating_add(button);
    let end = bounds.bottom.saturating_sub(button);
    let travel = i64::from(end) - i64::from(start);
    let range = i64::from(model.maximum) - i64::from(model.minimum);
    let length = if range == 0 {
        travel
    } else {
        let proportional = if range > i64::from(i32::MAX / 2) {
            0
        } else {
            travel * i64::from(model.page) / (range + i64::from(model.page)).max(1)
        };
        proportional.max((18.0 * model.scale).round() as i64).min(travel)
    };
    let offset = if range == 0 {
        0
    } else {
        ((i64::from(model.position) - i64::from(model.minimum)) * (travel - length) + range / 2)
            / range
    };
    let top = start.saturating_add(offset as i32);
    let bottom = top.saturating_add(length as i32);
    let painted_width = ((width as f32 / 2.0) * f32::from(model.hover) / 255.0)
        .max((2.0 * model.scale) as f32)
        .min(width as f32);
    let painted_center = left as f32 + width as f32 - painted_width;
    let inset_x = (2.0 * model.scale).round() as i32;
    let inset_top = (2.0 * model.scale).round() as i32;
    let inset_bottom = (3.0 * model.scale).round() as i32;
    Ok(Geometry {
        track: RECT { left, top: start, right: bounds.right, bottom: end },
        decrement: RECT {
            left: left.saturating_add(inset_x),
            top: bounds.top.saturating_add(inset_top),
            right: bounds.right.saturating_sub(inset_x),
            bottom: start.saturating_sub(inset_bottom),
        },
        increment: RECT {
            left: left.saturating_add(inset_x),
            top: end.saturating_add(inset_top),
            right: bounds.right.saturating_sub(inset_x),
            bottom: bounds.bottom.saturating_sub(inset_bottom),
        },
        thumb: RECT { left, top, right: bounds.right, bottom },
        paint_thumb: D2D_RECT_F {
            left: painted_center - painted_width / 2.0,
            top: top as f32,
            right: painted_center + painted_width / 2.0,
            bottom: bottom as f32,
        },
    })
}

fn contains(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

/// Resolve pointer input against unchanged full-width native geometry, including page regions.
pub(super) fn hit_test(model: &Model, point: POINT) -> Result<Part, DesktopError> {
    let native = geometry(model)?;
    if !model.enabled || model.minimum == model.maximum {
        return Ok(Part::None);
    }
    for (rect, part) in [
        (native.decrement, Part::Decrement),
        (native.increment, Part::Increment),
        (native.thumb, Part::Thumb),
    ] {
        if contains(rect, point) {
            return Ok(part);
        }
    }
    if contains(native.track, point) {
        return Ok(if point.y < native.thumb.top { Part::PageBefore } else { Part::PageAfter });
    }
    Ok(Part::None)
}

/// Map an owned thumb drag back into the original native range, clamping outside the track.
pub(super) fn drag_position(
    model: &Model,
    pointer_y: i32,
    grab_offset: i32,
) -> Result<i32, DesktopError> {
    let native = geometry(model)?;
    let travel = i64::from(native.track.bottom)
        - i64::from(native.track.top)
        - (i64::from(native.thumb.bottom) - i64::from(native.thumb.top));
    if travel <= 0 {
        return Ok(model.minimum);
    }
    let offset = (i64::from(pointer_y) - i64::from(grab_offset) - i64::from(native.track.top))
        .clamp(0, travel);
    let range = i64::from(model.maximum) - i64::from(model.minimum);
    Ok((i64::from(model.minimum) + (offset * range + travel / 2) / travel) as i32)
}

/// Publish only translucent thumb and system-font arrows; no track or background is cleared.
/// The caller owns BeginDraw/EndDraw and must observe the native EndDraw HRESULT.
pub(super) fn draw(target: &ID2D1RenderTarget, model: &Model) -> Result<(), DesktopError> {
    let native = geometry(model)?;
    if model.minimum == model.maximum {
        return Ok(());
    }
    let component = if model.dark { 1.0 } else { 0.0 };
    let color = D2D1_COLOR_F {
        r: component,
        g: component,
        b: component,
        a: if model.dark { 139.0 / 255.0 } else { 114.0 / 255.0 },
    };
    // SAFETY: The target and local immutable color remain owned for synchronous brush creation.
    let brush = unsafe { target.CreateSolidColorBrush(&color, None) }
        .map_err(|cause| DesktopError::new("console_scroll_brush", cause))?;
    // Pressing retains the system thumb color; its engine owner separately retains capture.
    let _pressed = model.pressed;
    // SAFETY: The local shape and owned brush remain valid until this draw call returns.
    unsafe {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: native.paint_thumb,
                radiusX: (3.0 * model.scale) as f32,
                radiusY: (3.0 * model.scale) as f32,
            },
            &brush,
        )
    };
    if model.hover > 153 {
        // SAFETY: The factory is retained locally and used only on this native UI thread.
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
            .map_err(|cause| DesktopError::new("console_scroll_font_factory", cause))?;
        // SAFETY: System family and locale literals remain live through creation.
        let format = unsafe {
            factory.CreateTextFormat(
                w!("Segoe Fluent Icons"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                (8.0 * model.scale) as f32,
                w!(""),
            )
        }
        .map_err(|cause| DesktopError::new("console_scroll_font", cause))?;
        // SAFETY: The owned format is UI-thread-affine and not shared during configuration.
        unsafe {
            format
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)
                .map_err(|cause| DesktopError::new("console_scroll_font_alignment", cause))?;
            format
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
                .map_err(|cause| DesktopError::new("console_scroll_font_alignment", cause))?;
            for (rect, glyph) in [(native.decrement, 0xeddb), (native.increment, 0xeddc)] {
                target.DrawText(
                    &[glyph],
                    &format,
                    &D2D_RECT_F {
                        left: rect.left as f32,
                        top: rect.top as f32,
                        right: rect.right as f32,
                        bottom: rect.bottom as f32,
                    },
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Model {
        Model {
            bounds: RECT { left: 0, top: 0, right: 12, bottom: 160 },
            scale: 1.0,
            minimum: 0,
            maximum: 900,
            page: 100,
            position: 0,
            dark: true,
            enabled: true,
            hover: 0,
            pressed: Part::None,
        }
    }

    #[test]
    fn native_range_and_hover_endpoints_keep_stable_input_geometry() -> Result<(), DesktopError> {
        let base = model();
        let resting = geometry(&base)?;
        let expanded = geometry(&Model { hover: 255, ..base })?;
        assert_eq!(resting.thumb, RECT { left: 0, top: 12, right: 12, bottom: 30 });
        assert_eq!(resting.paint_thumb.left, 9.0);
        assert_eq!(resting.paint_thumb.right, 11.0);
        assert_eq!(expanded.paint_thumb.left, 3.0);
        assert_eq!(expanded.paint_thumb.right, 9.0);
        assert_eq!(resting.thumb, expanded.thumb);
        assert_eq!(hit_test(&base, POINT { x: 0, y: 15 })?, Part::Thumb);
        assert_eq!(hit_test(&base, POINT { x: 5, y: 5 })?, Part::Decrement);
        assert_eq!(hit_test(&base, POINT { x: 5, y: 140 })?, Part::PageAfter);
        assert_eq!(drag_position(&base, -100, 0)?, 0);
        assert_eq!(drag_position(&base, 1000, 0)?, 900);
        Ok(())
    }

    #[test]
    fn scaling_tiny_ranges_and_extreme_positions_are_bounded() -> Result<(), DesktopError> {
        for scale in [0.5, 1.0, 1.75, 2.0] {
            let base = Model {
                bounds: RECT {
                    left: 0,
                    top: 0,
                    right: (12.0 * scale) as i32,
                    bottom: (160.0 * scale) as i32,
                },
                scale,
                ..model()
            };
            let first = geometry(&base)?;
            let last = geometry(&Model { position: 900, ..base })?;
            assert_eq!(first.thumb.top, first.track.top);
            assert_eq!(last.thumb.bottom, last.track.bottom);
        }
        let tiny = Model { bounds: RECT { left: 0, top: 0, right: 12, bottom: 8 }, ..model() };
        assert_eq!(drag_position(&tiny, i32::MAX, i32::MIN)?, 0);
        let huge = Model {
            minimum: i32::MIN,
            maximum: i32::MAX,
            position: i32::MIN,
            page: u32::MAX,
            ..model()
        };
        let thumb = geometry(&huge)?.thumb;
        assert_eq!(thumb.bottom - thumb.top, 18);
        assert!(geometry(&Model { scale: f64::NAN, ..model() }).is_err());
        assert!(geometry(&Model { minimum: 10, maximum: 0, ..model() }).is_err());
        Ok(())
    }
}
