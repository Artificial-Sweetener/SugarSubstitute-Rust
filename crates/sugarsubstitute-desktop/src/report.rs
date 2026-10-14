// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own localized native incident presentation independently of webviews and backends.
mod contract;
/// Test-only pure request metadata from the actual surface state owner.
#[cfg(feature = "boundary-fixtures")]
pub mod copy_fixture;
mod error;
#[cfg(target_os = "windows")]
mod preparation;
mod state;
mod view;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows;
pub use contract::*;
pub use error::{ReportSurfaceError, ReportSurfaceFailure};
#[cfg(target_os = "windows")]
pub use preparation::{ReportSessionPreparation, run_prepared_report_surface};
pub use view::{NativeReportView, ReportSurfaceOptions};
#[cfg(target_os = "windows")]
pub use windows::run_report_surface;
