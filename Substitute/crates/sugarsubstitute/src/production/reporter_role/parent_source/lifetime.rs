// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe genuine native facts and return original inputs only after checked family closure.
use super::*;
impl ReporterSource {
    /// Drain bounded native facts promptly; no wire parser/router runs on this native path.
    pub(in crate::production) fn poll(&self) -> Result<ReporterFacts, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        let changed = state.poll()?;
        let parent = state.parent.as_ref().ok_or(ReporterSourceIssue::Busy)?;
        Ok(ReporterFacts {
            changed,
            failed: parent.failed() || state.first.is_some(),
            authenticated: parent.authenticated(),
            process: parent.process(),
            pipe_settled: parent.pipe_settled(),
        })
    }
    /// Observe actual checked retirement independently of a retained native failure.
    /// Only an explicitly closed source may project these facts after a failed poll.
    pub(in crate::production) fn poll_retiring(
        &self,
    ) -> Result<ReporterFacts, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if !state.closing {
            return Err(ReporterSourceIssue::Identity);
        }
        let changed = match state.poll() {
            Ok(changed) => changed,
            Err(ReporterSourceIssue::Parent) => {
                state.first.get_or_insert(ReporterSourceIssue::Parent);
                false
            }
            Err(cause) => return Err(cause),
        };
        let parent = state.parent.as_ref().ok_or(ReporterSourceIssue::Busy)?;
        Ok(ReporterFacts {
            changed,
            failed: parent.failed() || state.first.is_some(),
            authenticated: parent.authenticated(),
            process: parent.process(),
            pipe_settled: parent.pipe_settled(),
        })
    }
    /// Request closure without dropping a view, rejected frame or unsettled native parent.
    pub(in crate::production) fn close(&self) -> Result<(), ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        state.closing = true;
        state.parent.as_ref().ok_or(ReporterSourceIssue::Busy)?.close();
        Ok(())
    }
    /// Return original completed inputs only after actual family closure; preserve all first causes.
    /// Core delivery and external repository cleanup must already be checked by the controller.
    pub(in crate::production) fn return_closed_inputs(
        &self,
        registry: &WindowsProcessRegistry,
    ) -> Result<bool, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if !state.closing {
            return Err(ReporterSourceIssue::Identity);
        }
        if !state
            .parent
            .as_mut()
            .ok_or(ReporterSourceIssue::Busy)?
            .return_closed_inputs(registry)
            .map_err(|_| ReporterSourceIssue::Parent)?
        {
            return Ok(false);
        }
        drop(state.rejected.take());
        drop(state.output.take());
        drop(state.completion.take());
        Ok(true)
    }
    /// Observe original causes outside the mutex; the callback cannot move their native custody.
    pub(in crate::production) fn failure<R>(
        &self,
        read: impl FnOnce(Option<&(dyn std::error::Error + 'static)>) -> R,
    ) -> Result<R, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        let parent = state.parent.as_ref().and_then(|parent| parent.failure());
        let cause: Option<&(dyn std::error::Error + 'static)> = match state.first {
            Some(ReporterSourceIssue::Clock) => state.time_cause.as_ref().map(|cause| cause as _),
            Some(ReporterSourceIssue::Wire) => state.wire.as_ref().map(|cause| cause as _),
            Some(ReporterSourceIssue::Parent) | None => parent,
            Some(_) => state.first.as_ref().map(|cause| cause as _),
        };
        let cause = state
            .registry_cause
            .as_ref()
            .map(|cause| cause as &(dyn std::error::Error + 'static))
            .or(cause)
            .or_else(|| state.first.as_ref().map(|cause| cause as _));
        Ok(read(cause))
    }
}
