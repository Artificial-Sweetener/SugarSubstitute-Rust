// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that composition failure selects windowed hosting only after retirement.

use super::*;

#[test]
fn ready_surface_selects_composition_hosting() -> Result<(), DesktopError> {
    let mut selection = HostingSelection::new();
    assert_eq!(selection.selected(), None);
    selection.surface_ready()?;
    assert_eq!(selection.selected(), Some(MainHosting::Composition));
    assert!(MainHosting::Composition.presents_native_canvas());
    Ok(())
}

#[test]
fn failed_surface_falls_back_to_windowed_only_after_retirement() -> Result<(), DesktopError> {
    let mut selection = HostingSelection::new();
    selection.surface_failed()?;
    assert_eq!(selection.selected(), None, "partial composition must retire first");
    selection.partial_retired()?;
    assert_eq!(selection.selected(), Some(MainHosting::Windowed));
    assert!(!MainHosting::Windowed.presents_native_canvas());
    Ok(())
}

#[test]
fn selected_hosting_never_changes() -> Result<(), DesktopError> {
    let mut composition = HostingSelection::new();
    composition.surface_ready()?;
    assert!(composition.surface_failed().is_err());
    assert!(composition.partial_retired().is_err());
    assert!(composition.surface_ready().is_err());
    assert_eq!(composition.selected(), Some(MainHosting::Composition));

    let mut windowed = HostingSelection::new();
    windowed.surface_failed()?;
    assert!(windowed.surface_ready().is_err(), "a failed surface cannot become composition");
    windowed.partial_retired()?;
    assert!(windowed.surface_failed().is_err());
    assert_eq!(windowed.selected(), Some(MainHosting::Windowed));
    Ok(())
}

#[test]
fn windowed_fallback_requires_an_observed_failure() {
    let mut selection = HostingSelection::new();
    assert!(selection.partial_retired().is_err());
    assert_eq!(selection.selected(), None);
}
