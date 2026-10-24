// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply prepared-control availability to the original retained HWND and callback mapping.
use super::*;
use sugarsubstitute_ui::availability::{AvailabilityPort, reconcile};
use windows::Win32::UI::WindowsAndMessaging::IsWindow;

/// Borrow one validated peer without changing its native identity or allocating storage.
pub(super) fn update(
    callback: &Callback,
    parent: Option<&Registration<Callback>>,
    peer: &Peer,
    mode: PaintMode,
    control: AccessibleControl,
) -> Result<(), DesktopError> {
    reconcile(
        peer.snapshot.availability(),
        control.availability(),
        &mut Visibility { callback, parent, peer, mode, control },
    )
}

/// Exact original native resources remain with Controls across every reentrant effect.
struct Visibility<'a> {
    callback: &'a Callback,
    parent: Option<&'a Registration<Callback>>,
    peer: &'a Peer,
    mode: PaintMode,
    control: AccessibleControl,
}
impl AvailabilityPort for Visibility<'_> {
    type Error = DesktopError;
    fn live(&self) -> Result<(), DesktopError> {
        if self.parent.is_none_or(|parent| !parent.installed() || parent.destroyed())
            || self
                .peer
                .registration
                .as_ref()
                .is_none_or(|peer| !peer.installed() || peer.destroyed())
        {
            return Err(DesktopError::message(
                "update_accessible_visibility",
                "original callback registration retired",
            ));
        }
        // SAFETY: The creating thread retains the original peer and callback registrations.
        if !unsafe { IsWindow(Some(self.peer.hwnd)) }.as_bool() {
            return Err(DesktopError::message(
                "update_accessible_visibility",
                "original native peer retired",
            ));
        }
        Ok(())
    }
    fn action(&mut self, available: bool) -> Result<(), DesktopError> {
        self.callback.set_action(
            self.peer.hwnd,
            self.control.id,
            available.then(|| super::action(self.mode, self.control)).flatten(),
        )
    }
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError> {
        sugarsubstitute_ui::windows::availability::set_enabled(self.peer.hwnd, enabled)?;
        self.live()
    }
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError> {
        sugarsubstitute_ui::windows::availability::set_visible(self.peer.hwnd, visible)?;
        self.live()
    }
}
