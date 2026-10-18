// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate semantic colors inside one guarded native paint without changing layout or text.
use super::super::render_proxy::{MAX_PAINT_COLORS, PaintColor, Proxy};
use super::Engine;
use crate::{DesktopError, console::index::DocumentIndex};
use sugarsubstitute_contracts::{configuration::Theme, presentation::console_color};
use windows::{
    Win32::{
        Foundation::{LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Direct2D::ID2D1RenderTarget,
        UI::Controls::RichEdit::*,
    },
    core::Interface,
};

const TOKEN_BASE: u32 = 0x005a_0000;

/// Complete scalar native scroll facts, independent of a stock scrollbar window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Scroll {
    pub(super) minimum: i32,
    pub(super) maximum: i32,
    pub(super) position: i32,
    pub(super) page: i32,
    pub(super) enabled: bool,
}
impl Scroll {
    /// Read the actual retained engine's current scroll range and viewport.
    pub(super) fn observe(services: &ITextServices2) -> Result<Self, DesktopError> {
        let mut result = Self { minimum: 0, maximum: 0, position: 0, page: 0, enabled: false };
        let mut enabled = windows::core::BOOL::default();
        // SAFETY: The caller retains this affine engine and all documented writable outputs.
        unsafe {
            services.TxGetVScroll(
                &mut result.minimum,
                &mut result.maximum,
                &mut result.position,
                &mut result.page,
                &mut enabled,
            )
        }
        .map_err(|cause| DesktopError::new("console_scroll_observe", cause))?;
        result.enabled = enabled.as_bool();
        if result.maximum < result.minimum || result.page < 0 {
            return Err(rejected("console_scroll_observe"));
        }
        Ok(result)
    }
    /// Empty and short documents have no remaining vertical content to follow.
    pub(super) fn at_bottom(self) -> bool {
        !self.enabled
            || self.maximum == self.minimum
            || i64::from(self.position) + i64::from(self.page) >= i64::from(self.maximum)
    }
}

/// Preserve scalar input state; format restoration uses the original shared authoritative index.
struct Original {
    selection: CHARRANGE,
    scroll: Scroll,
    pixel: POINT,
}
impl Original {
    fn observe(services: &ITextServices2) -> Result<Self, DesktopError> {
        let mut selection = CHARRANGE::default();
        send(services, EM_EXGETSEL, WPARAM(0), LPARAM((&raw mut selection) as isize))?;
        let mut pixel = POINT::default();
        send(services, EM_GETSCROLLPOS, WPARAM(0), LPARAM((&raw mut pixel) as isize))?;
        Ok(Self { selection, scroll: Scroll::observe(services)?, pixel })
    }
    fn restore(&self, services: &ITextServices2) -> Result<(), DesktopError> {
        send(services, EM_EXSETSEL, WPARAM(0), LPARAM((&raw const self.selection) as isize))?;
        send(services, EM_SETSCROLLPOS, WPARAM(0), LPARAM((&raw const self.pixel) as isize))?;
        let observed = Self::observe(services)?;
        if observed.selection.cpMin != self.selection.cpMin
            || observed.selection.cpMax != self.selection.cpMax
            || observed.scroll != self.scroll
            || observed.pixel != self.pixel
        {
            return Err(rejected("console_paint_restore_input"));
        }
        Ok(())
    }
}

/// Draw through exact semantic tokens, then verify complete restoration before pixels can publish.
pub(super) fn draw(
    engine: &Engine,
    source: &DocumentIndex,
    dark: bool,
    target: &ID2D1RenderTarget,
) -> Result<(), DesktopError> {
    engine.check()?;
    if engine.state.painting.replace(true) {
        return Err(rejected("console_paint_reentry"));
    }
    engine.state.observing_blocked.set(true);
    let result = draw_guarded(engine, source, dark, target);
    if result.is_ok() {
        engine.state.painting.set(false);
        engine.state.observing_blocked.set(!engine.state.redraw.get());
    }
    result
}

fn draw_guarded(
    engine: &Engine,
    source: &DocumentIndex,
    dark: bool,
    target: &ID2D1RenderTarget,
) -> Result<(), DesktopError> {
    let services = engine.services()?;
    let original = Original::observe(&services)?;
    // SAFETY: The same native engine implements TOM; these temporary ranges never escape paint.
    let document: ITextDocument =
        services.cast().map_err(|cause| DesktopError::new("console_paint_document", cause))?;
    let theme = if dark { Theme::Dark } else { Theme::Light };
    let mut colors = Vec::new();
    let count = source
        .styles()
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(4))
        .filter(|n| *n <= MAX_PAINT_COLORS)
        .ok_or_else(|| rejected("console_paint_capacity"))?;
    colors
        .try_reserve_exact(count)
        .map_err(|cause| DesktopError::new("console_paint_allocate", cause))?;
    if colors.capacity() != count {
        return Err(rejected("console_paint_capacity"));
    }
    let default = console_color(Default::default(), theme, false)
        .ok_or_else(|| rejected("console_paint_palette"))?;
    colors.push(PaintColor { token: TOKEN_BASE, rgba: default });
    colors.push(PaintColor { token: TOKEN_BASE + 1, rgba: [233, 30, 99, 107] });
    colors.push(PaintColor { token: TOKEN_BASE + 2, rgba: [255, 255, 255, 249] });
    engine.state.default_foreground.set(TOKEN_BASE);
    engine.state.selection_background.set(TOKEN_BASE + 1);
    engine.state.selection_foreground.set(TOKEN_BASE + 2);
    let tagged = (|| {
        set_colors(&services, 0, source.units as i32, TOKEN_BASE as i32, tomAutoColor.0)?;
        for (index, extent) in source.styles().iter().enumerate() {
            let token = TOKEN_BASE + 4 + index as u32 * 2;
            let foreground = console_color(extent.style.foreground, theme, false)
                .ok_or_else(|| rejected("console_paint_palette"))?;
            colors.push(PaintColor { token, rgba: foreground });
            let background = console_color(extent.style.background, theme, true);
            if let Some(rgba) = background {
                colors.push(PaintColor { token: token + 1, rgba });
            }
            set_colors(
                &services,
                extent.start,
                extent.end,
                token as i32,
                background.map_or(tomAutoColor.0, |_| (token + 1) as i32),
            )?;
        }
        original.restore(&services)?;
        let proxy = Proxy::new(target.clone(), colors)?;
        let bounds = engine.state.client_bounds();
        let mut logical = windows::Win32::Foundation::RECTL {
            left: bounds.left,
            top: bounds.top,
            right: bounds.right,
            bottom: bounds.bottom,
        };
        let mut update = bounds;
        // SAFETY: This is the sole native document and caller-owned transparent D2D transaction.
        unsafe {
            services.TxDrawD2D(proxy.interface(), &mut logical, &mut update, TXTVIEW_ACTIVE.0)
        }
        .map_err(|cause| DesktopError::new("console_native_alpha_draw", cause))?;
        proxy.finish()?;
        engine.state.check()
    })();
    // Restoration is attempted even after tagging, lookup or native draw fails. No synthetic
    // formatting may escape the guard; failed restoration leaves observation closed through close.
    let restored = restore_colors(&services, &document, source, theme)
        .and_then(|()| original.restore(&services));
    match (tagged, restored) {
        (Err(cause), Err(restoration)) => {
            tracing::error!(operation="console_paint_restore", error=%restoration, "Native format restoration failed after an earlier paint error");
            Err(cause)
        }
        (Err(cause), Ok(())) => Err(cause),
        (Ok(()), result) => result,
    }
}

fn restore_colors(
    services: &ITextServices2,
    document: &ITextDocument,
    source: &DocumentIndex,
    theme: Theme,
) -> Result<(), DesktopError> {
    let default = console_color(Default::default(), theme, false)
        .ok_or_else(|| rejected("console_paint_palette"))?;
    set_colors(services, 0, source.units as i32, rgb(default), tomAutoColor.0)?;
    for extent in source.styles() {
        let foreground = console_color(extent.style.foreground, theme, false)
            .ok_or_else(|| rejected("console_paint_palette"))?;
        let background =
            console_color(extent.style.background, theme, true).map_or(tomAutoColor.0, rgb);
        set_colors(services, extent.start, extent.end, rgb(foreground), background)?;
    }
    // Inspect each native source run after restoration. Only colors were changed; native font,
    // shaping, word wrap and bidi ownership never moved to a second layout engine.
    for extent in source.styles() {
        let foreground = console_color(extent.style.foreground, theme, false)
            .ok_or_else(|| rejected("console_paint_palette"))?;
        let background =
            console_color(extent.style.background, theme, true).map_or(tomAutoColor.0, rgb);
        verify_colors(document, extent.start, extent.end, rgb(foreground), background)?;
    }
    Ok(())
}

fn set_colors(
    services: &ITextServices2,
    start: i32,
    end: i32,
    foreground: i32,
    background: i32,
) -> Result<(), DesktopError> {
    let selection = CHARRANGE { cpMin: start, cpMax: end };
    send(services, EM_EXSETSEL, WPARAM(0), LPARAM((&raw const selection) as isize))?;
    let mut actual = CHARRANGE::default();
    send(services, EM_EXGETSEL, WPARAM(0), LPARAM((&raw mut actual) as isize))?;
    if actual.cpMin != start || actual.cpMax != end {
        return Err(rejected("console_paint_range"));
    }
    let mut format = CHARFORMAT2W::default();
    format.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
    format.Base.dwMask = CFM_COLOR | CFM_BACKCOLOR;
    format.Base.crTextColor = windows::Win32::Foundation::COLORREF(foreground as u32);
    if background == tomAutoColor.0 {
        format.Base.dwEffects = CFE_AUTOBACKCOLOR;
    } else {
        format.crBackColor = windows::Win32::Foundation::COLORREF(background as u32);
    }
    if send(
        services,
        EM_SETCHARFORMAT,
        WPARAM(SCF_SELECTION as usize),
        LPARAM((&raw const format) as isize),
    )?
    .0 == 0
    {
        return Err(rejected("console_paint_format"));
    }
    Ok(())
}
fn verify_colors(
    document: &ITextDocument,
    start: i32,
    end: i32,
    foreground: i32,
    background: i32,
) -> Result<(), DesktopError> {
    // SAFETY: Inspect the same original checked source run; no borrowed range survives this call.
    let (fore, back) = unsafe {
        let font = document
            .Range(start, end)
            .and_then(|range| range.GetFont())
            .map_err(|cause| DesktopError::new("console_paint_verify", cause))?;
        (font.GetForeColor(), font.GetBackColor())
    };
    if fore.map_err(|cause| DesktopError::new("console_paint_verify", cause))? != foreground
        || back.map_err(|cause| DesktopError::new("console_paint_verify", cause))? != background
    {
        return Err(rejected("console_paint_restore_format"));
    }
    Ok(())
}
fn send(
    services: &ITextServices2,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Result<LRESULT, DesktopError> {
    let mut result = LRESULT(0);
    // SAFETY: Every caller supplies a documented synchronous stack request to the same engine.
    let status = unsafe {
        (services.vtable().base__.TxSendMessage)(
            services.as_raw(),
            message,
            wparam,
            lparam,
            &mut result,
        )
    };
    status.ok().map_err(|cause| DesktopError::new("console_paint_message", cause))?;
    if status == windows::Win32::Foundation::S_FALSE {
        return Err(rejected("console_paint_message"));
    }
    Ok(result)
}
fn rgb([r, g, b, _]: [u8; 4]) -> i32 {
    i32::from(r) | (i32::from(g) << 8) | (i32::from(b) << 16)
}
fn rejected(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "native console paint contract rejected")
}
