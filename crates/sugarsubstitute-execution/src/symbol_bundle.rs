// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Seal installed symbol selection to its real paired original main and cleanup jobs.
mod admission;
mod origin;
mod purpose;
mod reader;
mod retirement;
pub use origin::{SymbolBundleAdmission, SymbolBundleOrigin};
pub use reader::{CommittedReaderAdmission, CommittedReaderOrigin, CommittedReaderRetirement};
pub use retirement::SymbolBundleRetirement;
#[cfg(test)]
mod reader_tests;

#[cfg(test)]
mod admission_tests;
#[cfg(test)]
mod retirement_tests;
#[cfg(test)]
mod test_support;
