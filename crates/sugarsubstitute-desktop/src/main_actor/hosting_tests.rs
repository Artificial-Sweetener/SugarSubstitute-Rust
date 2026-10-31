// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that every composition failure selects windowed hosting only after retirement.

use super::*;

#[test]
fn bound_browser_selects_composition_hosting() -> Result<(), DesktopError> {
    let mut selection = HostingSelection::new();
    assert_eq!((selection.construction(), selection.selected()), (None, None));
    selection.surface_ready()?;
    assert_eq!(selection.construction(), Some(MainHosting::Composition));
    assert_eq!(selection.selected(), None, "the browser is not bound yet");
    selection.browser_bound()?;
    assert_eq!(selection.selected(), Some(MainHosting::Composition));
    assert!(MainHosting::Composition.presents_native_canvas());
    Ok(())
}

#[test]
fn failed_surface_falls_back_to_windowed_only_after_retirement() -> Result<(), DesktopError> {
    let mut selection = HostingSelection::new();
    selection.surface_failed()?;
    assert_eq!((selection.construction(), selection.selected()), (None, None));
    selection.partial_retired()?;
    assert_eq!(selection.construction(), Some(MainHosting::Windowed));
    assert_eq!(selection.selected(), Some(MainHosting::Windowed));
    assert!(!MainHosting::Windowed.presents_native_canvas());
    Ok(())
}

#[test]
fn failed_bind_rebuilds_windowed_only_after_the_composition_browser_retires()
-> Result<(), DesktopError> {
    let mut selection = HostingSelection::new();
    selection.surface_ready()?;
    selection.bind_failed()?;
    assert_eq!(
        (selection.construction(), selection.selected()),
        (None, None),
        "no browser may be constructed while the unbound one retires"
    );
    assert!(selection.browser_bound().is_err());
    selection.composition_browser_retired()?;
    assert_eq!(selection.construction(), Some(MainHosting::Windowed));
    assert_eq!(selection.selected(), Some(MainHosting::Windowed));
    Ok(())
}

#[test]
fn selected_hosting_never_changes() -> Result<(), DesktopError> {
    let mut composition = HostingSelection::new();
    composition.surface_ready()?;
    composition.browser_bound()?;
    assert!(composition.bind_failed().is_err());
    assert!(composition.surface_failed().is_err());
    assert!(composition.partial_retired().is_err());
    assert_eq!(composition.selected(), Some(MainHosting::Composition));

    let mut windowed = HostingSelection::new();
    windowed.surface_failed()?;
    assert!(windowed.surface_ready().is_err(), "a failed surface cannot become composition");
    windowed.partial_retired()?;
    assert!(windowed.surface_failed().is_err());
    assert!(windowed.bind_failed().is_err());
    assert_eq!(windowed.selected(), Some(MainHosting::Windowed));
    Ok(())
}

#[test]
fn windowed_fallback_requires_an_observed_failure() {
    let mut selection = HostingSelection::new();
    assert!(selection.partial_retired().is_err());
    assert!(selection.composition_browser_retired().is_err());
    assert!(selection.browser_bound().is_err());
    assert_eq!(selection.selected(), None);
}
