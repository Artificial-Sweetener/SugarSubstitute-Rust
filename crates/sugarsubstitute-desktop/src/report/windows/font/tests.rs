// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fault the native font boundary while testing actual bounded custody and replacement.
use super::*;
use std::error::Error;
#[derive(Default)]
struct Fake {
    created: Vec<u32>,
    destroyed: Vec<HFONT>,
    fail_create: bool,
    fail_destroy: bool,
}
impl Boundary for Fake {
    fn create(&mut self, dpi: u32) -> Result<HFONT, ReportSurfaceError> {
        self.created.push(dpi);
        if self.fail_create {
            return Err(ReportSurfaceError::rejected("actual_font_create"));
        }
        Ok(HFONT(self.created.len() as *mut std::ffi::c_void))
    }
    fn destroy(&mut self, font: HFONT) -> Result<(), ReportSurfaceError> {
        self.destroyed.push(font);
        if self.fail_destroy {
            Err(ReportSurfaceError::rejected("actual_font_destroy"))
        } else {
            Ok(())
        }
    }
}
fn fonts() -> Fonts<Fake> {
    Fonts { boundary: Fake::default(), current: None, pending: None, dpi: 0, pending_dpi: 0 }
}
#[test]
fn native_report_font_changes_dpi_after_actual_widget_switch_and_checked_old_retirement()
-> Result<(), Box<dyn Error>> {
    let mut fonts = fonts();
    let first = fonts.prepare(96)?.ok_or("missing first font")?;
    assert!(fonts.current.is_none());
    fonts.commit()?;
    assert_eq!(fonts.current, Some(first));
    assert!(fonts.prepare(96)?.is_none());
    assert_eq!(fonts.boundary.created, [96]);
    let second = fonts.prepare(192)?.ok_or("missing DPI font")?;
    assert_eq!(fonts.current, Some(first));
    assert_eq!(fonts.pending, Some(second));
    fonts.commit()?;
    assert_eq!(fonts.current, Some(second));
    assert_eq!(fonts.boundary.destroyed, [first]);
    fonts.retire_after_children()?;
    assert!(fonts.current.is_none());
    assert_eq!(fonts.boundary.destroyed, [first, second]);
    Ok(())
}
#[test]
fn native_report_font_partial_replacement_holds_both_fonts_and_rejects_third_allocation()
-> Result<(), Box<dyn Error>> {
    let mut fonts = fonts();
    fonts.prepare(96)?;
    fonts.commit()?;
    fonts.prepare(144)?;
    fonts.boundary.fail_destroy = true;
    let error = fonts.commit().err().ok_or("fault not observed")?;
    assert_eq!(error.operation(), "actual_font_destroy");
    assert!(error.source().is_some());
    assert!(fonts.current.is_some());
    assert!(fonts.pending.is_some());
    assert!(fonts.prepare(192).is_err());
    assert_eq!(fonts.boundary.created, [96, 144]);
    fonts.boundary.fail_destroy = false;
    fonts.retire_after_children()?;
    assert!(fonts.current.is_none());
    assert!(fonts.pending.is_none());
    Ok(())
}
#[test]
fn native_report_font_preflights_invalid_dpi_and_preserves_create_or_retirement_failure()
-> Result<(), Box<dyn Error>> {
    let mut fonts = fonts();
    for dpi in [0, 47, 769, u32::MAX] {
        assert!(fonts.prepare(dpi).is_err());
    }
    assert!(fonts.boundary.created.is_empty());
    fonts.boundary.fail_create = true;
    assert!(fonts.prepare(96).is_err());
    assert!(fonts.pending.is_none());
    fonts.boundary.fail_create = false;
    fonts.prepare(96)?;
    fonts.boundary.fail_destroy = true;
    assert!(fonts.retire_after_children().is_err());
    assert!(fonts.pending.is_some());
    assert!(fonts.prepare(144).is_err());
    Ok(())
}
