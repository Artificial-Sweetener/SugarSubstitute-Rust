// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own finite native report export through the central admission and blocking pool.

pub(crate) mod admission;
pub(crate) mod cleanup;
mod origin;
pub(crate) mod pair;
#[cfg(test)]
mod pair_admission_tests;
#[cfg(test)]
mod pair_control_tests;
pub(crate) mod pair_run;
#[cfg(test)]
mod pair_test_support;
#[cfg(test)]
mod pair_tests;
pub(crate) mod resources;
mod retirement;
pub use admission::ReportExportSubmissionRejection;
pub use cleanup::{
    ReportExportCleanupBeginRejection, ReportExportCleanupDeadline, ReportExportCleanupPermit,
};
pub use origin::{ReportExportAdmission, ReportExportOrigin, ReportExportOutput};
pub use pair::{ReportExportPairJobs, ReportExportPairRequest, ReportExportPairSubmission};
pub use retirement::ReportExportRetirement;

/// Fixed complete main envelope, including streaming, native pins and metadata.
pub const REPORT_EXPORT_BYTES: u64 = 131_072;
/// Independent close-only cleanup envelope admitted before the main factory.
pub const REPORT_EXPORT_CLEANUP_BYTES: u64 = 32_768;
/// Maximum physical outputs held by one original export, including parent pins.
pub const REPORT_EXPORT_OUTPUTS: u16 = 68;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod control_tests;
