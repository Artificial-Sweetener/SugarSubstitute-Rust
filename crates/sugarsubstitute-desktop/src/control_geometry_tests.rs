// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Behavioral fixtures for the owning presentation contract.
use super::*;
use crate::ControlBounds;

/// Keep accessible peers aligned to their painted fractional-DPI bounds and reject unsafe input.
#[test]
fn native_controls_validate_identity_labels_and_fractional_geometry() -> Result<(), DesktopError> {
    let control = AccessibleControl {
        enabled: true,
        id: 1,
        label: "Show Comfy output",
        bounds: ControlBounds { x: 466.0, y: 0.0, width: 46.0, height: 32.0 },
        close: false,
        visible: true,
    };
    assert_eq!(
        physical_bounds(&control, 1.25)?,
        RECT { left: 583, top: 0, right: 640, bottom: 40 }
    );
    assert!(validate_controls(&[control, control]).is_err());
    assert!(validate_controls(&[AccessibleControl { id: 0, ..control }]).is_err());
    assert!(
        validate_controls(&[AccessibleControl { label: "invalid\0label", ..control }]).is_err()
    );
    for scale in [0.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(physical_bounds(&control, scale).is_err());
    }
    assert!(
        physical_bounds(
            &AccessibleControl {
                bounds: ControlBounds { x: f64::NEG_INFINITY, ..control.bounds },
                ..control
            },
            1.0
        )
        .is_err()
    );
    Ok(())
}
