// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the composed browser's rasterization scale, raw-pixel bounds and the native canvas
//! allocation on the main window's own DPI.
//!
//! The host owns the browser's scale: WebView2's monitor detection is disabled and every window
//! DPI change sets `RasterizationScale` before Tao applies the new client size. Bounds stay in
//! raw physical pixels, so the page's device pixel ratio is the rasterization scale times the
//! page zoom. The page republishes its geometry when that ratio or its layout changes. A resize
//! keeps the latest native pixels on screen until a frame at the new geometry replaces them; a
//! density change withdraws them rather than showing them at a stale size.

use super::*;
use crate::canvas::placement::{PhysicalRect, WebViewAllocation};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_BOUNDS_MODE_USE_RAW_PIXELS, ICoreWebView2Controller3,
};
use windows::Win32::Foundation::{HWND, POINT, RECT};

/// The browser controller's scale, zoom and bounds: the WebView2 adapter boundary.
///
/// Implementations are UI-thread affine and must be called on the controller's apartment.
pub(super) trait Browser {
    /// Set the browser's rasterization scale (physical pixels per unzoomed CSS pixel).
    fn set_rasterization_scale(&self, scale: f64) -> windows::core::Result<()>;
    /// The browser's current rasterization scale.
    fn rasterization_scale(&self) -> windows::core::Result<f64>;
    /// The page zoom applied on top of the rasterization scale.
    fn zoom_factor(&self) -> windows::core::Result<f64>;
    /// The browser's bounds in raw physical pixels of the host window's client area.
    fn bounds(&self) -> windows::core::Result<RECT>;
}

impl Browser for ICoreWebView2Controller3 {
    fn set_rasterization_scale(&self, scale: f64) -> windows::core::Result<()> {
        // SAFETY: The retained controller belongs to the calling UI apartment.
        unsafe { self.SetRasterizationScale(scale) }
    }
    fn rasterization_scale(&self) -> windows::core::Result<f64> {
        let mut scale = 0.0;
        // SAFETY: The retained controller copies one scalar into live local storage.
        unsafe { self.RasterizationScale(&raw mut scale) }?;
        Ok(scale)
    }
    fn zoom_factor(&self) -> windows::core::Result<f64> {
        let mut zoom = 0.0;
        // SAFETY: The retained controller copies one scalar into live local storage.
        unsafe { self.ZoomFactor(&raw mut zoom) }?;
        Ok(zoom)
    }
    fn bounds(&self) -> windows::core::Result<RECT> {
        let mut bounds = RECT::default();
        // SAFETY: The retained controller copies one rectangle into live local storage.
        unsafe { self.Bounds(&raw mut bounds) }?;
        Ok(bounds)
    }
}

/// Apply the window's scale to the browser.
///
/// # Errors
/// Rejects a scale that is not positive and finite, and preserves the controller's failure.
pub(super) fn rescale(browser: &impl Browser, scale: f64) -> Result<(), DesktopError> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(DesktopError::message(
            "rescale_composed_browser",
            "window scale is not positive and finite",
        ));
    }
    browser
        .set_rasterization_scale(scale)
        .map_err(|cause| DesktopError::new("rescale_composed_browser", cause))
}

/// Observe the browser's actual raw-pixel bounds in the main window's client area, whose
/// host window's client origin sits at `origin` there, and its device pixel ratio.
///
/// # Errors
/// Preserves controller getter failures and rejects inverted or unrepresentable bounds.
pub(super) fn allocation(
    browser: &impl Browser,
    origin: POINT,
) -> Result<WebViewAllocation, DesktopError> {
    let observe = |cause| DesktopError::new("observe_composed_browser_allocation", cause);
    let bounds = browser.bounds().map_err(observe)?;
    let scale = browser.rasterization_scale().map_err(observe)?;
    let zoom = browser.zoom_factor().map_err(observe)?;
    let extent = |start: i32, end: i32| {
        u32::try_from(i64::from(end) - i64::from(start)).map_err(|_| {
            DesktopError::message("observe_composed_browser_allocation", "browser bounds inverted")
        })
    };
    let place = |offset: i32, start: i32| {
        offset.checked_add(start).ok_or_else(|| {
            DesktopError::message("observe_composed_browser_allocation", "browser origin overflows")
        })
    };
    Ok(WebViewAllocation {
        bounds: PhysicalRect {
            x: place(origin.x, bounds.left)?,
            y: place(origin.y, bounds.top)?,
            width: extent(bounds.left, bounds.right)?,
            height: extent(bounds.top, bounds.bottom)?,
        },
        physical_pixels_per_css_pixel: scale * zoom,
    })
}

/// Whether shown native pixels were placed in a coordinate space the browser no longer has.
///
/// A browser resized about the same origin and density lays its page out in the same physical
/// client pixels, so the latest frame stays on screen until one at the new geometry replaces
/// it, and Windows clips anything beyond a shrunken client area. An origin or density change
/// would show the pixels at the wrong place or size, so it withdraws them.
pub(super) fn stale(shown: Option<WebViewAllocation>, current: WebViewAllocation) -> bool {
    shown.is_some_and(|shown| {
        [shown.bounds.x, shown.bounds.y] != [current.bounds.x, current.bounds.y]
            || shown.physical_pixels_per_css_pixel.to_bits()
                != current.physical_pixels_per_css_pixel.to_bits()
    })
}

impl super::Composition {
    /// Take scale ownership from WebView2: raw-pixel bounds at the window's current scale,
    /// without the browser's own monitor detection.
    pub(super) fn adopt_scale(&mut self) -> Result<(), DesktopError> {
        let controller = self.controller.as_ref().ok_or_else(|| missing("browser_controller"))?;
        let scaled = controller
            .cast::<ICoreWebView2Controller3>()
            .map_err(|cause| DesktopError::new("configure_composed_browser_scale", cause))?;
        self.scale = Some(scaled.clone());
        let window = self.window.as_ref().ok_or_else(|| missing("composition_window"))?;
        // SAFETY: The retained controller belongs to this UI apartment.
        unsafe { scaled.SetBoundsMode(COREWEBVIEW2_BOUNDS_MODE_USE_RAW_PIXELS) }
            // SAFETY: As above.
            .and_then(|()| unsafe { scaled.SetShouldDetectMonitorScaleChanges(false) })
            .map_err(|cause| DesktopError::new("configure_composed_browser_scale", cause))?;
        rescale(&scaled, window.scale_factor())
    }

    /// The scale-owning controller, available once composition construction adopted it.
    fn scaled(&self) -> Result<&ICoreWebView2Controller3, DesktopError> {
        self.scale.as_ref().ok_or_else(|| missing("composition_browser_scale"))
    }

    /// The page's top-left in the main window's client pixels.
    fn page_origin(&self) -> Result<POINT, DesktopError> {
        let controller = self.controller.as_ref().ok_or_else(|| missing("browser_controller"))?;
        let window = self.window.as_ref().ok_or_else(|| missing("composition_window"))?;
        super::page::client_origin(
            super::page::host(controller)?,
            HWND(window.hwnd() as *mut std::ffi::c_void),
        )
    }

    /// Draw the bound browser where its host window now places the page: a composed browser
    /// draws at its visual's origin, not at its bounds. Returns whether the page moved.
    fn place_browser(&mut self) -> Result<bool, DesktopError> {
        if self.stage != super::Stage::Ready || !self.ledger.browser_bound || self.retired {
            return Ok(false);
        }
        let origin = self.page_origin()?;
        if self.placed == Some(origin) {
            return Ok(false);
        }
        let (browser, device) = (self.browser()?, self.device()?);
        // SAFETY: The retained visual and device belong to this UI apartment; the commit is a
        // synchronous request on the device that owns the visual.
        #[allow(clippy::cast_precision_loss)]
        let placed = unsafe {
            browser
                .SetOffsetX2(origin.x as f32)
                .and_then(|()| browser.SetOffsetY2(origin.y as f32))
                .and_then(|()| device.Commit())
        };
        placed.map_err(|cause| DesktopError::new("place_composed_browser", cause))?;
        self.placed = Some(origin);
        Ok(true)
    }
}

impl super::MainActor {
    /// Rescale the browser for a window DPI change, before Tao applies its new client size.
    pub(in crate::main_actor) fn rescale_composition(
        &self,
        scale: f64,
    ) -> Result<(), DesktopError> {
        match self.composition.as_ref().and_then(|tree| tree.scale.as_ref()) {
            Some(scaled) => rescale(scaled, scale),
            None => Ok(()),
        }
    }

    /// Draw the composed browser where `resize` placed its host window, and tell the browser
    /// its page moved so popups, IME candidates and accessibility bounds follow it.
    pub(in crate::main_actor) fn place_composed_browser(&mut self) -> Result<(), DesktopError> {
        let Some(tree) = self.composition.as_mut() else { return Ok(()) };
        if tree.place_browser()? {
            self.composition_moved()?;
        }
        Ok(())
    }

    /// Observe the actual browser allocation the page lays out against.
    pub(crate) fn canvas_allocation(&self) -> Result<WebViewAllocation, DesktopError> {
        let tree = self.composition.as_ref().ok_or_else(|| missing("browser_composition"))?;
        allocation(tree.scaled()?, tree.page_origin()?)
    }

    /// Withdraw native pixels and their input region once the browser's origin or density
    /// changed under them, so no frame stays on screen at the wrong place or size.
    pub(in crate::main_actor) fn withdraw_stale_canvas(&mut self) -> Result<(), DesktopError> {
        let Some(shown) = self.composition.as_ref().and_then(|tree| tree.shown) else {
            return Ok(());
        };
        if !stale(Some(shown), self.canvas_allocation()?) {
            return Ok(());
        }
        if let Some(input) = self.composition.as_ref().and_then(|tree| tree.input.as_ref()) {
            input.publish(None);
        }
        self.hide_canvas()?;
        Ok(())
    }

    /// Tell the browser its host window moved, so popups, IME candidates and accessibility
    /// bounds follow the window on screen.
    pub(in crate::main_actor) fn composition_moved(&self) -> Result<(), DesktopError> {
        if self.composition.is_none() {
            return Ok(());
        }
        // SAFETY: The retained controller belongs to this UI apartment.
        unsafe { self.view()?.controller().NotifyParentWindowPositionChanged() }
            .map_err(|cause| DesktopError::new("notify_composed_browser_moved", cause))
    }
}

#[cfg(test)]
#[path = "scale/tests.rs"]
mod tests;
