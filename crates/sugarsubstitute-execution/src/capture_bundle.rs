// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Seal original capture bundle accounting and close-only retirement to actual Core custody.

mod admission;
#[cfg(any(windows, test))]
mod desktop_events;
#[cfg(any(windows, test))]
mod desktop_retirement;
#[cfg(any(windows, test))]
pub(crate) mod desktop_transfer;
#[cfg(any(windows, test))]
mod desktop_unissued;
mod origin;
mod retirement;
pub(crate) mod validate;

pub use origin::{CaptureBundleAdmission, CaptureBundleOrigin};
pub use retirement::CaptureBundleRetirement;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod scope_tests;
