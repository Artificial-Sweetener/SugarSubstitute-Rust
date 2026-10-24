// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve fixed peer identities and fence native activation across availability changes.
use crate::{AccessibleControl, DesktopError};

/// Validate the original stable inventory before any native update effect.
pub(super) fn validate_inventory(
    planned: &[AccessibleControl],
    controls: &[AccessibleControl],
) -> Result<(), DesktopError> {
    if controls.len() != planned.len() {
        return Err(DesktopError::message(
            "update_accessible_controls",
            "control inventory changed during the surface lifetime",
        ));
    }
    for (control, original) in controls.iter().zip(planned) {
        if control.id != original.id || control.close != original.close {
            return Err(DesktopError::message(
                "update_accessible_controls",
                "control identity or native action changed",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_controls_state_tests.rs"]
mod tests;
