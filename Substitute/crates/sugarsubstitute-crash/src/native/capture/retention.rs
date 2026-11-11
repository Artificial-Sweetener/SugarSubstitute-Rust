// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original bounded native causes through partial closure and observation.
use super::*;
impl Fields {
    /// Retain each actual owned cause once in the fixed ledger or omitted count.
    pub(super) fn retain(&mut self, cause: CrashError) {
        if let Some(slot) = self.errors.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    /// Adopt checked partial-closure causes before reporting the original first cause.
    pub(super) fn close_report(&mut self, report: pins::CloseReport) -> Option<CrashError> {
        let first = report.first;
        if let Some(cause) = &first {
            self.retain(cause.clone());
        }
        for cause in report.additional.into_iter().flatten() {
            self.retain(cause);
        }
        self.omitted = self.omitted.saturating_add(report.omitted);
        first
    }
    /// Preserve supported v1 Win32 cleanup causes without changing their domain.
    pub(super) fn cleanup_codes(&mut self, output: &bridge::ResultValue) -> Result<(), CrashError> {
        if output.cleanup_count > 8 {
            return Err(CrashError::invalid(CaptureOperation::Retirement));
        }
        for code in &output.cleanup_codes[..output.cleanup_count as usize] {
            self.retain(native_code(*code, CaptureOperation::Retirement));
        }
        self.omitted = self.omitted.saturating_add(output.omitted_cleanup);
        Ok(())
    }
}
/// Decode only the supported v1 Win32 cause; v2 keeps its separate typed domains.
pub(super) fn native_code(code: u32, operation: CaptureOperation) -> CrashError {
    CrashError::new(operation, std::io::Error::from_raw_os_error(code as i32))
}
