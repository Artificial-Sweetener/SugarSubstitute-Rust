// SPDX-License-Identifier: GPL-3.0-or-later
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
    sets: Cell<u32>,
    failing: bool,
}

impl Fake {
    fn new(scale: f64, zoom: f64, size: (i32, i32)) -> Self {
        Self {
            scale: Cell::new(scale),
            zoom,
            size: Cell::new(size),
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
        let (right, bottom) = self.size.get();
        Ok(RECT { left: 0, top: 0, right, bottom })
    }
}

#[test]
fn a_dpi_change_rescales_the_browser_and_moves_the_allocation_off_shown_pixels() -> TestResult {
    let browser = Fake::new(1.0, 1.0, (800, 600));
    let shown = allocation(&browser)?;
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
    let rescaled = allocation(&browser)?;
    assert_eq!(rescaled.physical_pixels_per_css_pixel, 1.5);
    assert!(stale(Some(shown), rescaled), "density alone invalidates shown pixels");

    browser.size.set((1200, 900));
    let resized = allocation(&browser)?;
    assert_eq!(resized.bounds, PhysicalRect { x: 0, y: 0, width: 1200, height: 900 });
    assert!(stale(Some(shown), resized));
    assert!(!stale(Some(resized), resized), "geometry acknowledged at the new allocation");
    assert!(!stale(None, resized), "nothing shown, nothing to withdraw");
    Ok(())
}

#[test]
fn page_zoom_multiplies_the_rasterization_scale() -> TestResult {
    let browser = Fake::new(1.25, 1.5, (100, 50));
    assert_eq!(allocation(&browser)?.physical_pixels_per_css_pixel, 1.875);
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
    assert!(allocation(&inverted).is_err());
}
