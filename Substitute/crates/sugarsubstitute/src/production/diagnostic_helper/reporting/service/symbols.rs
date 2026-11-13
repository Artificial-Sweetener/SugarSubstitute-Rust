// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight the same report service's bounded Symbols allocation before transfer.
use super::*;
impl ReportingService {
    /// Fund one bounded Symbols allocation from this existing16KiB role before taking input.
    /// Rejection leaves the same genuine observation in the caller's occupied slot.
    /// # Errors
    /// Rejects active/closed/aliased source custody and missing input; no new claim is created.
    pub(in crate::production) fn prepare_symbols(
        &self,
        binding: ReportBinding,
        observation: &mut Option<SymbolReportObservation>,
    ) -> Result<ReportingWork, ReportingIssue> {
        if self.installed
            || self.work.occupied()
            || self.presentation.occupied()
            || self.cleanup.occupied()
        {
            return Err(ReportingIssue::Busy);
        }
        let result = self.source.box_symbols(observation)?;
        Ok(ReportingWork::Symbols { binding, result })
    }
}
