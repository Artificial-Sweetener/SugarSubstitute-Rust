// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Windows widget painting adapters. Native windows and event loops remain desktop-owned.

#[allow(unsafe_code)]
pub mod availability;
#[allow(unsafe_code)]
pub mod caret;
#[allow(unsafe_code)]
pub mod font;
#[allow(unsafe_code)]
pub mod scroll_input;
pub mod scroll_port;
#[allow(unsafe_code)]
mod scroll_wheel;
#[allow(unsafe_code)]
pub mod scrollbar;
#[allow(unsafe_code)]
pub mod select;
pub mod text_style;

/// Preserve a native widget operation and its original error across the host boundary.
#[derive(Debug)]
pub struct NativeUiError {
    operation: &'static str,
    cause: Box<dyn std::error::Error>,
}

impl NativeUiError {
    /// Retain an actual native failure without replacing its diagnostic cause.
    pub fn new(operation: &'static str, cause: impl std::error::Error + 'static) -> Self {
        Self { operation, cause: Box::new(cause) }
    }

    /// Reject invalid widget input before a platform draw or control effect.
    pub(crate) fn message(operation: &'static str, message: &'static str) -> Self {
        Self::new(operation, std::io::Error::new(std::io::ErrorKind::InvalidInput, message))
    }

    /// Stable native operation identity for the desktop adapter's structured diagnostic.
    pub fn operation(&self) -> &'static str {
        self.operation
    }

    /// Preserve the first failure while subsequent native obligations still retire.
    pub(crate) fn retain_first(slot: &mut Option<Self>, cause: Self) {
        if slot.is_none() {
            *slot = Some(cause);
        }
    }
}

impl std::fmt::Display for NativeUiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.cause)
    }
}

impl std::error::Error for NativeUiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
