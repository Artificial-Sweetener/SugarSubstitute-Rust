// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the Windows system accent through WinRT `UISettings` and observe its changes.
//!
//! The settings object and its change registration belong to the native UI thread that
//! created them. Windows raises `ColorValuesChanged` on a system thread; that callback only
//! wakes the owning event loop, which then reads the accent and delivers each distinct value.

use crate::{
    completion_wake::{ProxyWake, WakeLifetime},
    native_action::NativeAction,
};
use sugarsubstitute_contracts::{appearance::AccentColor, execution::CompletionWake};
use tao::event_loop::EventLoopProxy;
use windows::{
    Foundation::TypedEventHandler,
    UI::{
        Color,
        ViewManagement::{UIColorType, UISettings},
    },
};

/// Observe the system accent for one native event loop and deliver each distinct value once.
///
/// An unavailable settings object is not an application failure: the accent is reported as
/// absent and native surfaces use the product palette. Retirement removes the registration;
/// it must precede disconnection of the owning loop's wake lifetime.
pub(crate) struct SystemAccent {
    registration: Option<(UISettings, i64)>,
    delivery: AccentDelivery,
}

impl SystemAccent {
    /// Register for accent changes that wake `proxy` with [`NativeAction::SystemAccent`].
    pub(crate) fn observe(proxy: EventLoopProxy<NativeAction>, lifetime: WakeLifetime) -> Self {
        let wake = ProxyWake::new(proxy, lifetime, NativeAction::SystemAccent);
        let handler =
            TypedEventHandler::<UISettings, windows_core::IInspectable>::new(move |_, _| {
                if let Err(cause) = wake.wake() {
                    // A retired loop disconnects its wake before the registration is removed.
                    tracing::debug!(operation = "system_accent_wake", cause = %cause,
                        "System accent change arrived after its event loop closed");
                }
                Ok(())
            });
        let registration = UISettings::new().and_then(|settings| {
            let token = settings.ColorValuesChanged(&handler)?;
            Ok((settings, token))
        });
        let registration = match registration {
            Ok(registration) => Some(registration),
            Err(cause) => {
                tracing::warn!(operation = "system_accent_observe", cause = %cause,
                    "Windows accent changes cannot be observed; the current accent is still read");
                None
            }
        };
        Self { registration, delivery: AccentDelivery::default() }
    }

    /// Read the current accent and return it only when it differs from the last delivery.
    /// The first call always returns the current value, including an absent accent.
    pub(crate) fn take_change(&mut self) -> Option<Option<AccentColor>> {
        let current = match &self.registration {
            Some((settings, _)) => read(settings),
            None => current(),
        };
        self.delivery.offer(current)
    }

    /// Remove the change registration; later notifications cannot reach the loop.
    pub(crate) fn retire(&mut self) {
        if let Some((settings, token)) = self.registration.take()
            && let Err(cause) = settings.RemoveColorValuesChanged(token)
        {
            tracing::warn!(operation = "system_accent_retire", cause = %cause,
                "Windows accent change registration could not be removed");
        }
    }
}

impl Drop for SystemAccent {
    fn drop(&mut self) {
        self.retire();
    }
}

/// Read the current system accent once, without observing later changes.
pub(crate) fn current() -> Option<AccentColor> {
    match UISettings::new() {
        Ok(settings) => read(&settings),
        Err(cause) => {
            tracing::warn!(operation = "system_accent_read", cause = %cause,
                "Windows accent settings are unavailable; using the product accent");
            None
        }
    }
}

fn read(settings: &UISettings) -> Option<AccentColor> {
    match settings.GetColorValue(UIColorType::Accent) {
        Ok(color) => Some(accent(color)),
        Err(cause) => {
            tracing::warn!(operation = "system_accent_read", cause = %cause,
                "Windows accent color is unavailable; using the product accent");
            None
        }
    }
}

/// Windows reports an opaque accent; its alpha channel carries no accent information.
fn accent(color: Color) -> AccentColor {
    AccentColor::new(color.R, color.G, color.B)
}

/// Coalesce repeated platform notifications, which Windows raises for every UI color change.
#[derive(Debug, Default)]
struct AccentDelivery {
    delivered: Option<Option<AccentColor>>,
}

impl AccentDelivery {
    fn offer(&mut self, current: Option<AccentColor>) -> Option<Option<AccentColor>> {
        if self.delivered == Some(current) {
            return None;
        }
        self.delivered = Some(current);
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unrelated color notifications deliver nothing; a real accent change is delivered once.
    #[test]
    fn delivery_reports_the_first_value_and_only_later_changes() {
        let blue = Some(AccentColor::new(0, 120, 212));
        let green = Some(AccentColor::new(16, 124, 16));
        let mut delivery = AccentDelivery::default();
        assert_eq!(delivery.offer(None), Some(None));
        assert_eq!(delivery.offer(None), None);
        assert_eq!(delivery.offer(blue), Some(blue));
        assert_eq!(delivery.offer(blue), None);
        assert_eq!(delivery.offer(green), Some(green));
        assert_eq!(delivery.offer(None), Some(None));
    }

    /// The WinRT color's alpha is discarded and its channels are retained exactly.
    #[test]
    fn winrt_color_maps_to_opaque_rgb_accent() {
        let color = Color { A: 0x7f, R: 0x00, G: 0x78, B: 0xd4 };
        assert_eq!(accent(color), AccentColor::new(0x00, 0x78, 0xd4));
    }
}
