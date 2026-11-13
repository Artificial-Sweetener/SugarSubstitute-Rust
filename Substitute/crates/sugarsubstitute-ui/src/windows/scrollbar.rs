// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint shared Windows scrollbar chrome over the authoritative native scroll range.
use super::NativeUiError;
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
pub enum Part {
    /// No interactive region.
    None,
    /// Scroll toward the beginning.
    Decrement,
    /// Scroll toward the end.
    Increment,
    /// Drag the current viewport.
    Thumb,
    /// Move one viewport toward the beginning.
    PageBefore,
    /// Move one viewport toward the end.
    PageAfter,
}

/// Presentation treatment; both variants use the same range, input and native painter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appearance {
    /// Transparent transcript chrome expands on hover.
    Overlay,
    /// A reserved page gutter keeps its track, thumb and arrows visible.
    Gutter,
}

/// Physical client bounds and native position facts; `dark` is the Windows style color scheme.
/// `maximum` is the last admissible position; `hover` is the parent's 100-ms animation, 0..255.
#[derive(Clone, Copy)]
pub struct Model {
    /// Shared treatment selected by the containing viewport.
    pub appearance: Appearance,
    /// Physical client rectangle containing the scrollbar.
    pub bounds: RECT,
    /// Logical-to-physical scale, finite and positive.
    pub scale: f64,
    /// First admissible position.
    pub minimum: i32,
    /// Last admissible position, after subtracting the viewport extent.
    pub maximum: i32,
    /// Viewport extent in the native document coordinate system.
    pub page: u32,
    /// Current position in the inclusive minimum/maximum range.
    pub position: i32,
    /// Select the system dark color treatment.
    pub dark: bool,
    /// Permit pointer interaction with the authoritative scroll range.
    pub enabled: bool,
    /// Sampled hover expansion, zero through 255.
    pub hover: u8,
    /// Region whose gesture is currently captured by the native adapter.
    pub pressed: Part,
}

/// Full-width hit regions remain stable while only the painted thumb expands.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    /// Physical track excluding arrow buttons.
    pub track: RECT,
    /// Physical upper-arrow hit rectangle.
    pub decrement: RECT,
    /// Physical lower-arrow hit rectangle.
    pub increment: RECT,
    /// Full-width physical thumb hit rectangle.
    pub thumb: RECT,
    /// Animated physical thumb paint rectangle; hit geometry remains stable.
    pub paint_thumb: D2D_RECT_F,
}

/// Resolve bounded native position facts into physical hit and publication geometry.
pub fn geometry(model: &Model) -> Result<Geometry, NativeUiError> {
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
        return Err(NativeUiError::message(
            "console_scroll_geometry",
            "invalid native scroll geometry",
        ));
    }
    let extent = (match model.appearance {
        Appearance::Overlay => 12.0,
        Appearance::Gutter => 15.0,
    } * model.scale)
        .round() as i32;
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
    let painted_width = if model.appearance == Appearance::Gutter {
        (width as f32 - (6.0 * model.scale) as f32).max(0.0)
    } else {
        ((width as f32 / 2.0) * f32::from(model.hover) / 255.0)
            .max((2.0 * model.scale) as f32)
            .min(width as f32)
    };
    let painted_center = left as f32
        + if model.appearance == Appearance::Gutter {
            width as f32 / 2.0
        } else {
            width as f32 - painted_width
        };
    let thumb_inset =
        if model.appearance == Appearance::Gutter { (3.0 * model.scale) as f32 } else { 0.0 }
            .min(length as f32 / 2.0);
    let arrow_top = ((if model.appearance == Appearance::Gutter { 3.0 } else { 2.0 }) * model.scale)
        .round() as i32;
    let inset_x = (2.0 * model.scale).round() as i32;
    let inset_top = (2.0 * model.scale).round() as i32;
    let inset_bottom = (3.0 * model.scale).round() as i32;
    Ok(Geometry {
        track: RECT { left, top: start, right: bounds.right, bottom: end },
        decrement: RECT {
            left: left.saturating_add(inset_x),
            top: bounds.top.saturating_add(arrow_top),
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
            top: top as f32 + thumb_inset,
            right: painted_center + painted_width / 2.0,
            bottom: bottom as f32 - thumb_inset,
        },
    })
}

fn contains(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

/// Resolve pointer input against unchanged full-width native geometry, including page regions.
pub fn hit_test(model: &Model, point: POINT) -> Result<Part, NativeUiError> {
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
pub fn drag_position(
    model: &Model,
    pointer_y: i32,
    grab_offset: i32,
) -> Result<i32, NativeUiError> {
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

/// Publish the selected shared treatment with system-font arrows.
/// The caller owns BeginDraw/EndDraw and must observe the native EndDraw HRESULT.
pub fn draw(target: &ID2D1RenderTarget, model: &Model) -> Result<(), NativeUiError> {
    let native = geometry(model)?;
    if model.minimum == model.maximum {
        return Ok(());
    }
    let color = foreground(model);
    if model.appearance == Appearance::Gutter {
        let shade = if model.dark { 44.0 / 255.0 } else { 241.0 / 255.0 };
        // SAFETY: This affine target and immutable palette remain live during native painting.
        let track = unsafe {
            target
                .CreateSolidColorBrush(&D2D1_COLOR_F { r: shade, g: shade, b: shade, a: 1.0 }, None)
        }
        .map_err(|cause| NativeUiError::new("scroll_track_brush", cause))?;
        // SAFETY: The locally owned brush and bounded geometry outlive the synchronous call.
        unsafe {
            target.FillRectangle(
                &D2D_RECT_F {
                    left: native.track.left as f32,
                    top: model.bounds.top as f32,
                    right: native.track.right as f32,
                    bottom: model.bounds.bottom as f32,
                },
                &track,
            )
        };
    }
    // SAFETY: The target and local immutable color remain owned for synchronous brush creation.
    let brush = unsafe { target.CreateSolidColorBrush(&color, None) }
        .map_err(|cause| NativeUiError::new("console_scroll_brush", cause))?;
    let radius = if model.appearance == Appearance::Gutter { 4.5 } else { 3.0 };
    // SAFETY: The local shape and owned brush remain valid until this draw call returns.
    unsafe {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: native.paint_thumb,
                radiusX: (radius * model.scale) as f32,
                radiusY: (radius * model.scale) as f32,
            },
            &brush,
        )
    };
    if model.appearance == Appearance::Gutter || model.hover > 153 {
        // SAFETY: The factory is retained locally and used only on this native UI thread.
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }
            .map_err(|cause| NativeUiError::new("console_scroll_font_factory", cause))?;
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
        .map_err(|cause| NativeUiError::new("console_scroll_font", cause))?;
        // SAFETY: The owned format is UI-thread-affine and not shared during configuration.
        unsafe {
            format
                .SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)
                .map_err(|cause| NativeUiError::new("console_scroll_font_alignment", cause))?;
            format
                .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
                .map_err(|cause| NativeUiError::new("console_scroll_font_alignment", cause))?;
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

/// Keep baseline system colors independent of the containing page's translucent material.
fn foreground(model: &Model) -> D2D1_COLOR_F {
    if model.appearance == Appearance::Gutter {
        let shade = if model.dark { 159.0 / 255.0 } else { 128.0 / 255.0 };
        D2D1_COLOR_F { r: shade, g: shade, b: shade, a: 1.0 }
    } else {
        let component = if model.dark { 1.0 } else { 0.0 };
        D2D1_COLOR_F {
            r: component,
            g: component,
            b: component,
            a: if model.dark { 139.0 / 255.0 } else { 114.0 / 255.0 },
        }
    }
}

#[cfg(test)]
#[path = "scrollbar_tests.rs"]
mod tests;
