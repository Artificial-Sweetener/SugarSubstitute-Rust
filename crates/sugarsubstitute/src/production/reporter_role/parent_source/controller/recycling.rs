// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate delivered relay custody before the same funded source is recycled.
use super::*;
impl ReporterRelay {
    /// Cached genuine funding/empty-state observation, granting no native retirement authority.
    pub(in crate::production) fn idle_funded(&self) -> bool {
        matches!(self.work, RelayWork::Idle)
            && self.original.is_none()
            && self.admission.is_none()
            && self.identity.is_none()
            && self.source.idle_funded()
    }
    fn recyclable_delivery(&self) -> Result<(), ReporterSourceIssue> {
        if matches!(self.work, RelayWork::Admitted(_)) {
            return Err(ReporterSourceIssue::Busy);
        }
        if matches!(self.work, RelayWork::Rejected(_)) || self.identity.is_some() {
            return Err(ReporterSourceIssue::Admission);
        }
        if let RelayWork::Delivered(done) = &self.work {
            if !self.matched {
                return Err(ReporterSourceIssue::Identity);
            }
            if !matches!(
                done.outcome,
                JobOutcome::Success(worker::RelayTurn::Advanced | worker::RelayTurn::Deferred)
            ) {
                return Err(ReporterSourceIssue::Parent);
            }
        }
        Ok(())
    }
    /// Require a genuinely delivered healthy relay and checked original native/family custody.
    pub(in crate::production) fn recycling_ready(
        &self,
        registry: &WindowsProcessRegistry,
        expected: DiagnosticKey,
    ) -> Result<bool, ReporterSourceIssue> {
        self.recyclable_delivery()?;
        self.source.recycling_ready(registry, expected)
    }
    /// Keep the same source closed while its detached original receipt is adopted.
    pub(in crate::production) fn detach_closed(
        &mut self,
        registry: &WindowsProcessRegistry,
        expected: DiagnosticKey,
    ) -> Result<Option<RecycledReporter>, ReporterSourceIssue> {
        self.recyclable_delivery()?;
        self.source.detach_closed(registry, expected)
    }
    /// Finish only the receiving owner's actual accepted disposition; never renew old deadlines.
    pub(in crate::production) fn finish_recycled(
        &mut self,
        receipt: &mut Option<RecycledReporter>,
    ) -> Result<DisposedReporter, ReporterSourceIssue> {
        self.recyclable_delivery()?;
        let disposition = self.source.finish_recycled(receipt)?;
        self.admission = None;
        self.work = RelayWork::Idle;
        self.terminal = None;
        self.original = None;
        self.matched = false;
        Ok(disposition)
    }
}
