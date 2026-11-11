// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check the original counter at affine clipboard entry and retain its first native cause.
use super::{buffer::Text, controls::Controls};
use crate::{
    report::{NativeReportView, ReportSurfaceError, ReportSurfaceFailure},
    timing::TimingClock,
    windows_timing::WindowsClock,
};
use sugarsubstitute_contracts::{
    execution::CounterDeadline, localization::incident::ReportMessages,
};
/// External native effects stay behind this synchronous affine boundary.
pub(super) trait CopyBoundary {
    /// Build charged immutable native text before sampling the final counter.
    fn prepare(&mut self) -> Result<(), ReportSurfaceError>;
    /// Read raw absolute ticks and actual frequency on the current affine thread.
    fn counter(&mut self) -> Result<(i64, i64), ReportSurfaceError>;
    fn transfer(&mut self) -> Result<(), ReportSurfaceError>;
    fn status(&mut self, success: bool) -> Result<(), ReportSurfaceError>;
}
pub(super) struct NativeCopy<'a> {
    controls: &'a mut Controls,
    view: &'a NativeReportView,
    labels: &'static ReportMessages,
    text: Option<Text>,
}
impl<'a> NativeCopy<'a> {
    pub(super) fn new(
        controls: &'a mut Controls,
        view: &'a NativeReportView,
        labels: &'static ReportMessages,
    ) -> Self {
        Self { controls, view, labels, text: None }
    }
}
impl CopyBoundary for NativeCopy<'_> {
    fn prepare(&mut self) -> Result<(), ReportSurfaceError> {
        self.text = Some(Text::new(self.view.text(), &self.controls.lease)?);
        Ok(())
    }
    fn counter(&mut self) -> Result<(i64, i64), ReportSurfaceError> {
        let mut clock = WindowsClock::new()?;
        let stamp = clock.read()?;
        if stamp.thread != clock.thread() || stamp.source != crate::timing::QPC_SOURCE {
            return Err(ReportSurfaceError::rejected("report_copy_clock_identity"));
        }
        Ok((stamp.ticks, clock.frequency()))
    }
    fn transfer(&mut self) -> Result<(), ReportSurfaceError> {
        let text = self
            .text
            .as_ref()
            .ok_or_else(|| ReportSurfaceError::rejected("report_copy_text_missing"))?;
        self.controls
            .clipboard
            .copy(&text.units, &self.controls.lease)
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_transfer", cause))
    }
    fn status(&mut self, success: bool) -> Result<(), ReportSurfaceError> {
        self.controls.status(if success { self.labels.copied } else { self.labels.copy_failed })
    }
}
/// No counter is translated or renewed here. Charged text preparation precedes the final QPC check.
pub(super) fn execute(
    boundary: &mut impl CopyBoundary,
    original: CounterDeadline,
    authorized: Result<(), ReportSurfaceFailure>,
    failure: &mut Option<ReportSurfaceError>,
    observe: impl FnOnce(Result<(), ReportSurfaceFailure>) -> Result<(), ReportSurfaceError>,
) -> Result<(), ReportSurfaceError> {
    let result = if authorized.is_ok() {
        boundary.prepare()?;
        let (ticks, frequency) = boundary.counter()?;
        if ticks <= 0 || frequency != original.frequency() || ticks >= original.ticks() {
            return Err(ReportSurfaceError::rejected("report_copy_original_counter_expired"));
        }
        boundary.transfer()
    } else {
        Err(ReportSurfaceError::rejected("report_copy_host_rejected"))
    };
    let success = result.is_ok();
    if let Err(cause) = result {
        if failure.is_none() {
            *failure = Some(cause);
        } else {
            tracing::error!(
                operation = cause.operation(),
                "Additional native Copy failure retained categorically"
            );
        }
    }
    boundary.status(success)?;
    observe(if success { Ok(()) } else { Err(ReportSurfaceFailure::Clipboard) })
}
#[cfg(test)]
mod tests;
