// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate native accessible identities and convert their logical painted bounds.

use crate::{AccessibleControl, DesktopError};
use windows::Win32::Foundation::RECT;

/// Reject invalid metadata before assigning native identities or reading coordinates.
pub(crate) fn validate_controls(controls: &[AccessibleControl]) -> Result<(), DesktopError> {
    let mut ids = std::collections::BTreeSet::new();
    for control in controls {
        if control.id == 0
            || !ids.insert(control.id)
            || control.label.is_empty()
            || control.label.contains('\0')
        {
            return Err(DesktopError::message(
                "validate_accessible_controls",
                "control IDs and labels must be nonempty and unique",
            ));
        }
        physical_bounds(control, 1.0)?;
    }
    Ok(())
}

/// Preserve fractional DPI and reject nonfinite, empty or overflowing native control rectangles.
pub(crate) fn physical_bounds(
    control: &AccessibleControl,
    scale: f64,
) -> Result<RECT, DesktopError> {
    let bounds = control.bounds;
    let coordinates = [bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height]
        .map(|value| (value * scale).round());
    if !scale.is_finite()
        || scale <= 0.0
        || bounds.width <= 0.0
        || bounds.height <= 0.0
        || coordinates.iter().any(|value| {
            !value.is_finite() || *value < f64::from(i32::MIN) || *value > f64::from(i32::MAX)
        })
    {
        return Err(DesktopError::message(
            "validate_accessible_bounds",
            "control geometry is nonfinite, empty or outside native coordinate bounds",
        ));
    }
    let [left, top, right, bottom] = coordinates.map(|value| value as i32);
    if right.checked_sub(left).is_none_or(|span| span <= 0)
        || bottom.checked_sub(top).is_none_or(|span| span <= 0)
    {
        return Err(DesktopError::message(
            "validate_accessible_bounds",
            "control dimensions round to empty or overflow native bounds",
        ));
    }
    Ok(RECT { left, top, right, bottom })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ControlBounds;

    /// Keep accessible peers aligned to their painted fractional-DPI bounds and reject unsafe input.
    #[test]
    fn native_controls_validate_identity_labels_and_fractional_geometry() -> Result<(), DesktopError>
    {
        let control = AccessibleControl {
            id: 1,
            label: "Show Comfy output",
            bounds: ControlBounds { x: 466.0, y: 0.0, width: 46.0, height: 32.0 },
            close: false,
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
}
