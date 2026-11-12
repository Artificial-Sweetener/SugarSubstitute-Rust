// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe authoritative native document scroll facts.
use crate::DesktopError;
use windows::Win32::UI::Controls::RichEdit::ITextServices2;
/// Complete scalar native scroll facts, independent of a stock scrollbar window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Scroll {
    pub(crate) minimum: i32,
    pub(crate) maximum: i32,
    pub(crate) position: i32,
    pub(crate) page: i32,
    pub(crate) enabled: bool,
}
impl Scroll {
    /// Read the actual retained engine's current scroll range and viewport.
    pub(crate) fn observe(services: &ITextServices2) -> Result<Self, DesktopError> {
        let mut result = Self { minimum: 0, maximum: 0, position: 0, page: 0, enabled: false };
        let mut enabled = windows::core::BOOL::default();
        // SAFETY: The caller retains this affine engine and all documented writable outputs.
        unsafe {
            services.TxGetVScroll(
                &mut result.minimum,
                &mut result.maximum,
                &mut result.position,
                &mut result.page,
                &mut enabled,
            )
        }
        .map_err(|cause| DesktopError::new("console_scroll_observe", cause))?;
        result.enabled = enabled.as_bool();
        if result.maximum < result.minimum || result.page < 0 {
            return Err(DesktopError::message(
                "console_scroll_observe",
                "Invalid native scroll range",
            ));
        }
        Ok(result)
    }
    /// Empty and short documents have no remaining vertical content to follow.
    pub(crate) fn at_bottom(self) -> bool {
        !self.enabled
            || self.maximum == self.minimum
            || i64::from(self.position) + i64::from(self.page) >= i64::from(self.maximum)
    }
}
