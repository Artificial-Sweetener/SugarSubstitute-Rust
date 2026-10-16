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

/// Replace only native availability effects; peer identity and transition ordering stay here.
pub(super) trait VisibilityPort {
    /// Revalidate the same retained peer and callback registrations after Win32 reentry.
    fn live(&self) -> Result<(), DesktopError>;
    /// Fence or restore the actual retained callback action without a borrow across native calls.
    fn action(&mut self, available: bool) -> Result<(), DesktopError>;
    /// Apply and verify enabled state; the native API returns previous state rather than success.
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError>;
    /// Apply and verify the peer's own visibility bit independently of parent visibility.
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError>;
}

/// Reconcile only changed snapshot availability. Geometry-only resize leaves the current
/// native action and enabled state with their authoritative presentation owner.
pub(super) fn update_visibility(
    previous: AccessibleControl,
    current: AccessibleControl,
    port: &mut impl VisibilityPort,
) -> Result<(), DesktopError> {
    if previous.visible != current.visible {
        apply_visibility(port, current.visible)?;
    }
    Ok(())
}

/// Hidden peers remain disabled and have no action even for explicitly sent native messages.
/// Restoration publishes the action only after actual visibility and enabling succeed.
pub(super) fn apply_visibility(
    port: &mut impl VisibilityPort,
    visible: bool,
) -> Result<(), DesktopError> {
    port.live()?;
    port.action(false)?;
    port.enabled(false)?;
    port.live()?;
    port.visible(visible)?;
    port.live()?;
    if visible {
        port.enabled(true)?;
        port.live()?;
        port.action(true)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "windows_controls_state_tests.rs"]
mod tests;
