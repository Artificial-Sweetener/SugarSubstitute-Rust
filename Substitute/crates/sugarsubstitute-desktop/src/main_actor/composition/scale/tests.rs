// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive a window DPI change through a fake browser controller boundary.

use super::*;
use std::cell::Cell;
use windows::Win32::Foundation::E_FAIL;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A controller boundary fake: Tao resizes the client area after the scale change.
struct Fake {
    scale: Cell<f64>,
    zoom: f64,
    size: Cell<(i32, i32)>,
    /// The bounds' top within the host window.
    top: Cell<i32>,
    sets: Cell<u32>,
    failing: bool,
}

impl Fake {
    fn new(scale: f64, zoom: f64, size: (i32, i32)) -> Self {
        Self {
            scale: Cell::new(scale),
            zoom,
            size: Cell::new(size),
            top: Cell::new(0),
            sets: Cell::new(0),
            failing: false,
        }
    }
}

impl Browser for Fake {
    fn set_rasterization_scale(&self, scale: f64) -> windows::core::Result<()> {
        if self.failing {
            return Err(windows::core::Error::from(E_FAIL));
        }
        self.sets.set(self.sets.get() + 1);
        self.scale.set(scale);
        Ok(())
    }
    fn rasterization_scale(&self) -> windows::core::Result<f64> {
        Ok(self.scale.get())
    }
    fn zoom_factor(&self) -> windows::core::Result<f64> {
        Ok(self.zoom)
    }
    fn bounds(&self) -> windows::core::Result<RECT> {
        let (right, height) = self.size.get();
        let top = self.top.get();
        Ok(RECT { left: 0, top, right, bottom: top + height })
    }
}

#[test]
fn a_dpi_change_withdraws_shown_pixels_but_a_resize_keeps_them() -> TestResult {
    let browser = Fake::new(1.0, 1.0, (800, 600));
    let shown = allocation(&browser, POINT::default())?;
    assert_eq!(
        shown,
        WebViewAllocation {
            bounds: PhysicalRect { x: 0, y: 0, width: 800, height: 600 },
            physical_pixels_per_css_pixel: 1.0,
        }
    );
    assert!(!stale(Some(shown), shown));

    rescale(&browser, 1.5)?;
    assert_eq!(browser.sets.get(), 1);
    let rescaled = allocation(&browser, POINT::default())?;
    assert_eq!(rescaled.physical_pixels_per_css_pixel, 1.5);
    assert!(stale(Some(shown), rescaled), "density alone invalidates shown pixels");

    browser.size.set((1200, 900));
    let resized = allocation(&browser, POINT::default())?;
    assert_eq!(resized.bounds, PhysicalRect { x: 0, y: 0, width: 1200, height: 900 });
    assert!(
        !stale(Some(rescaled), resized),
        "a resize keeps shown pixels until a frame at the new geometry replaces them"
    );
    assert!(!stale(None, resized), "nothing shown, nothing to withdraw");
    Ok(())
}

#[test]
fn a_moved_browser_origin_withdraws_shown_pixels() {
    let shown = WebViewAllocation {
        bounds: PhysicalRect { x: 0, y: 0, width: 800, height: 600 },
        physical_pixels_per_css_pixel: 1.0,
    };
    let moved = WebViewAllocation { bounds: PhysicalRect { x: 0, y: 34, ..shown.bounds }, ..shown };
    assert!(stale(Some(shown), moved));
}

#[test]
fn the_allocation_starts_where_the_host_window_places_the_page() -> TestResult {
    let browser = Fake::new(1.75, 1.0, (2240, 1340));
    let below_caption = allocation(&browser, POINT { x: 0, y: 60 })?;
    assert_eq!(below_caption.bounds, PhysicalRect { x: 0, y: 60, width: 2240, height: 1340 });
    let filling = allocation(&browser, POINT::default())?;
    assert!(stale(Some(below_caption), filling), "the page moving back withdraws shown pixels");
    assert!(allocation(&browser, POINT { x: 0, y: i32::MAX }).is_ok());
    let browser = Fake::new(1.0, 1.0, (10, 10));
    browser.top.set(1);
    assert!(allocation(&browser, POINT { x: 0, y: i32::MAX }).is_err(), "an overflowing origin");
    Ok(())
}

#[test]
fn page_zoom_multiplies_the_rasterization_scale() -> TestResult {
    let browser = Fake::new(1.25, 1.5, (100, 50));
    assert_eq!(allocation(&browser, POINT::default())?.physical_pixels_per_css_pixel, 1.875);
    Ok(())
}

#[test]
fn unrepresentable_scales_and_controller_failures_fail_closed() {
    let browser = Fake::new(1.0, 1.0, (100, 50));
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(rescale(&browser, scale).is_err(), "{scale}");
    }
    assert_eq!((browser.sets.get(), browser.scale.get()), (0, 1.0));
    let failing = Fake { failing: true, ..Fake::new(1.0, 1.0, (100, 50)) };
    assert!(rescale(&failing, 2.0).is_err());
    let inverted = Fake::new(1.0, 1.0, (-1, 50));
    assert!(allocation(&inverted, POINT::default()).is_err());
}
