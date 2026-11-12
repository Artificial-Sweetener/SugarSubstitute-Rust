// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe retained job lifetimes before transferring any original authenticated input.
use super::*;

impl ExportService {
    /// Observe already fixed admission deadlines without issuing or renewing a token.
    pub(in crate::production) fn next_deadline(&self) -> Option<Moment> {
        if self.receiving_deadline.is_some() || self.source_deadline.is_some() {
            return self
                .receiving_deadline
                .map(|deadline| deadline.deadline())
                .into_iter()
                .chain(self.source_deadline.map(|deadline| deadline.deadline()))
                .min();
        }
        self.delivery[..2]
            .iter()
            .filter_map(|delivery| delivery.job.as_ref())
            .filter_map(|job| job.spec().deadline)
            .min()
    }
    /// Observe actual cleanup rejection, including typed false from a retained native close error.
    pub(in crate::production) fn cleanup_failed(&self) -> bool {
        self.delivery[2..]
            .iter()
            .any(|delivery| delivery.completion.is_some() && !delivery.succeeded())
    }
    /// Observe actual Main delivery and both successful physical cleanup deliveries.
    /// This grants no later native effect; success remains separate in `receipt`.
    pub(in crate::production) fn checked_terminal(&self) -> bool {
        self.delivery_error.is_none()
            && self.delivery.iter().all(|delivery| delivery.completion.is_some())
            && self.delivery[2..].iter().all(Delivery::succeeded)
            && self
                .source
                .inspect(|operation| operation.source_closed && operation.receiving_closed)
                == Some(true)
    }
    /// Request cancellation of the two same preheld cleanup jobs, then observe their deliveries.
    /// Identical input restoration waits actual cancellation; no Main/cleanup success is invented.
    pub(in crate::production) fn take_unsubmitted_input(
        &mut self,
    ) -> Result<Option<ExportInput>, ExportIssue> {
        if !self.retire_unsubmitted()? {
            return Ok(None);
        }
        self.source.take_unsubmitted_input()
    }
    /// Cancel only unpaired held cleanup and await every real original cancellation delivery.
    /// This leaves the complete input in its source for admission-failure correlation.
    pub(in crate::production) fn retire_unsubmitted(&mut self) -> Result<bool, ExportIssue> {
        if self.delivery[..2]
            .iter()
            .any(|delivery| delivery.job.is_some() || delivery.completion.is_some())
        {
            return Err(ExportIssue::Started);
        }
        if let Some(permit) = self.receiving_held.take() {
            self.delivery[2].job = Some(permit.cancel_and_observe());
        }
        if let Some(permit) = self.source_held.take() {
            self.delivery[3].job = Some(permit.cancel_and_observe());
        }
        if self.delivery[2..].iter().any(|delivery| {
            delivery
                .completion
                .as_ref()
                .is_some_and(|done| !matches!(done.outcome, JobOutcome::Cancelled))
        }) {
            return Err(ExportIssue::Identity);
        }
        if !self.unsubmitted_observed() {
            return Ok(false);
        }
        Ok(true)
    }
    pub(super) fn unsubmitted_observed(&self) -> bool {
        self.delivery[..2]
            .iter()
            .all(|delivery| delivery.job.is_none() && delivery.completion.is_none())
            && self.delivery_error.is_none()
            && self.source_held.is_none()
            && self.receiving_held.is_none()
            && [(0, 3), (1, 2)].into_iter().all(|(role, lane)| {
                !self.cleanup_admitted[role]
                    || matches!(
                        self.delivery[lane].completion.as_ref().map(|done| &done.outcome),
                        Some(JobOutcome::Cancelled)
                    )
            })
    }
    /// Dispose original cancelled held deliveries only at the caller's actual family/pipe boundary.
    pub(in crate::production) fn release_unsubmitted(&mut self) -> Result<(), ExportIssue> {
        if !self.unsubmitted_observed() {
            return Err(ExportIssue::Busy);
        }
        self.source.release_unsubmitted()?;
        self.delivery = std::array::from_fn(|_| Delivery::default());
        self.cleanup_admitted = [false; 2];
        self.admission = None;
        self.delivery_error = None;
        Ok(())
    }
    /// Transfer the authenticated completion token only after both actual checked cleanup jobs.
    /// Failed or uncertain cleanup retains the original token and all source custody here.
    pub(in crate::production) fn take_terminal(&mut self) -> Option<ExportTerminal> {
        if !self.checked_terminal() {
            return None;
        }
        let receipt = self.receipt();
        let mut loan = self.source.loan(false).ok()?;
        let operation = loan.operation().ok()?;
        if !operation.source_closed || !operation.receiving_closed {
            return None;
        }
        let mut input = operation.take_terminal_input()?;
        let view = operation
            .export
            .as_mut()
            .and_then(ReportExportOperation::take_view)
            .or_else(|| input.view.take());
        Some(ExportTerminal {
            received: input.received,
            issued: input.issued,
            receipt,
            view,
            _source: self.source.alias(),
        })
    }
}
