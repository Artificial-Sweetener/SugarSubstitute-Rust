// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared native control availability with activation fenced across reentrant OS calls.

/// Independent visible and enabled presentation states for one retained native peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Availability {
    /// Whether the control remains visible within its parent's surface.
    pub visible: bool,
    /// Whether a visible control accepts input and exposes an available action.
    pub enabled: bool,
}

impl Availability {
    /// Hidden peers never admit actions, regardless of their retained enabled preference.
    pub fn actionable(self) -> bool {
        self.visible && self.enabled
    }
}

/// Affine effects on the original native peer, whose lifetime remains with the desktop adapter.
pub trait AvailabilityPort {
    /// Original native failure, preserved without converting it into a presentation success.
    type Error;
    /// Revalidate original child and callback custody after native reentry.
    fn live(&self) -> Result<(), Self::Error>;
    /// Remove or restore callback admission independently of the OS enabled bit.
    fn action(&mut self, available: bool) -> Result<(), Self::Error>;
    /// Set and verify the actual OS enabled bit.
    fn enabled(&mut self, enabled: bool) -> Result<(), Self::Error>;
    /// Set and verify child visibility independently of the parent's visibility.
    fn visible(&mut self, visible: bool) -> Result<(), Self::Error>;
}

/// Reconcile changed availability without touching native state for geometry-only updates.
/// # Errors
/// Retains the port's original failure; the caller must retain the old snapshot and peer custody.
pub fn reconcile<P: AvailabilityPort>(
    previous: Availability,
    current: Availability,
    port: &mut P,
) -> Result<(), P::Error> {
    if previous != current {
        apply(current, port)?;
    }
    Ok(())
}

/// Fence activation before native calls and restore it only after visible/enabled verification.
/// # Errors
/// Returns the original port error. Failed native effects never restore the callback action.
pub fn apply<P: AvailabilityPort>(current: Availability, port: &mut P) -> Result<(), P::Error> {
    port.live()?;
    port.action(false)?;
    port.enabled(false)?;
    port.live()?;
    port.visible(current.visible)?;
    port.live()?;
    if current.actionable() {
        port.enabled(true)?;
        port.live()?;
        port.action(true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
