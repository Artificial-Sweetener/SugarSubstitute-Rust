// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original native causes while keeping categorical presentation failures.
use std::{error::Error, fmt};
/// Closed action result; never implies filesystem durability or process settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportSurfaceFailure {
    /// Actual clipboard transfer failed.
    Clipboard,
    /// Native destination selection failed.
    SaveSelection,
    /// Native presentation failed.
    Presentation,
    /// Actual native teardown remains uncertain.
    Retirement,
    /// Actual durable metadata mutation failed.
    Store,
    /// Actual local export failed.
    Export,
}
/// Preserve original owner causes without exposing their strings as product copy.
pub struct ReportSurfaceError {
    operation: &'static str,
    cause: Box<dyn Error>,
}
impl ReportSurfaceError {
    /// Retain an actual adapter/content cause; this creates no native privileges.
    pub fn new(operation: &'static str, cause: impl Error + 'static) -> Self {
        Self { operation, cause: Box::new(cause) }
    }
    pub(super) fn rejected(operation: &'static str) -> Self {
        Self::new(operation, std::io::Error::other("Native report contract rejected"))
    }
    /// Borrow the stable developer diagnostic operation name.
    pub const fn operation(&self) -> &'static str {
        self.operation
    }
}
impl fmt::Debug for ReportSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReportSurfaceError")
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ReportSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native report operation failed: {}", self.operation)
    }
}
impl Error for ReportSurfaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl From<crate::DesktopError> for ReportSurfaceError {
    fn from(cause: crate::DesktopError) -> Self {
        Self::new("report_desktop_boundary", cause)
    }
}
/// Retain an actual unwound payload without formatting private panic text in product copy.
pub(super) struct PanicCause {
    _payload: Box<dyn std::any::Any + Send>,
}
impl PanicCause {
    pub(super) fn new(payload: Box<dyn std::any::Any + Send>) -> Self {
        Self { _payload: payload }
    }
}
impl fmt::Debug for PanicCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeReportContentPanic")
    }
}
impl fmt::Display for PanicCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Native report content unwound")
    }
}
impl Error for PanicCause {}

#[cfg(windows)]
impl From<sugarsubstitute_ui::windows::NativeUiError> for ReportSurfaceError {
    fn from(cause: sugarsubstitute_ui::windows::NativeUiError) -> Self {
        Self::new("report_native_ui", cause)
    }
}
