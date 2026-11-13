// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain genuine central deliveries separately from checked physical source and receiving closure.
use super::{source::Source, *};
use sugarsubstitute_execution::{
    CleanupDeadline, ExecutionClient, ReportExportCleanupDeadline, ReportExportCleanupPermit,
    ReportExportPairRequest, SymbolCleanupPermit,
};
/// Inert service decision; the service retains original failure and partial native custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum ExportIssue {
    Started,
    Identity,
    Busy,
    /// Actual bounded overload; unpaired cleanup must retire before failed-action adoption.
    Admission,
    /// Permanent original central rejection, retained for explicit source inspection.
    Rejected,
}
impl std::fmt::Display for ExportIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incident export service rejected: {self:?}")
    }
}
impl std::error::Error for ExportIssue {}
#[path = "service/delivery.rs"]
mod delivery;
use delivery::Delivery;
#[path = "service/cleanup.rs"]
mod cleanup;
#[path = "service/failure.rs"]
mod failure;
#[path = "service/lifecycle.rs"]
mod lifecycle;
#[path = "service/terminal.rs"]
mod terminal;
pub(in crate::production) use terminal::{
    ExportCompletionCustody, ExportCompletionParts, ExportTerminal,
};
/// Fixed controller source and four distinct original central deliveries.
/// It submits no second ordinary worker while source Main occupies the sole ordinary lane.
pub(in crate::production) struct ExportService {
    source: Source,
    delivery: [Delivery; 4],
    source_held: Option<SymbolCleanupPermit<bool, ExportIssue>>,
    receiving_held: Option<ReportExportCleanupPermit<bool, ExportIssue>>,
    source_deadline: Option<CleanupDeadline>,
    receiving_deadline: Option<ReportExportCleanupDeadline>,
    admission: Option<AdmissionError>,
    delivery_error: Option<ExportIssue>,
    cleanup_admitted: [bool; 2],
}
impl ExportService {
    /// Admit the fixed32KiB metadata from the already admitted controller2MiB.
    pub(in crate::production) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        Ok(Self {
            source: Source::new(context)?,
            delivery: std::array::from_fn(|_| Delivery::default()),
            source_held: None,
            receiving_held: None,
            source_deadline: None,
            receiving_deadline: None,
            admission: None,
            delivery_error: None,
            cleanup_admitted: [false; 2],
        })
    }
    /// Only central bounded overload permits an admission-only failed action after real cancellation.
    /// Permanent identity/scope/time/provider failures retain their original cause immediately.
    fn reject_admission(&mut self, cause: AdmissionError) -> ExportIssue {
        if self.admission.as_ref().is_some_and(|original| !Self::capacity_rejection(original)) {
            return ExportIssue::Rejected;
        }
        if Self::capacity_rejection(&cause) {
            self.admission.get_or_insert(cause);
            ExportIssue::Admission
        } else {
            // Overload is a deferred disposition; preserve the first permanent failure.
            self.admission = Some(cause);
            ExportIssue::Rejected
        }
    }
    /// Classify only actual bounded central overload; identity/time/provider errors remain strict.
    fn capacity_rejection(cause: &AdmissionError) -> bool {
        matches!(
            cause,
            AdmissionError::QueueSaturated { .. }
                | AdmissionError::CompletionSaturated
                | AdmissionError::MemorySaturated { .. }
        )
    }
    /// Move genuine input once, preserving it on later job/admission failure.
    pub(in crate::production) fn bind(
        &mut self,
        input: &mut Option<ExportInput>,
    ) -> Result<(), ExportIssue> {
        if self.cleanup_admitted.iter().any(|admitted| *admitted)
            || self.delivery.iter().any(|lane| lane.job.is_some() || lane.completion.is_some())
        {
            return Err(ExportIssue::Started);
        }
        self.source.install(input)
    }
    /// Actual central coordinated execution supplies both live original contexts on one worker.
    /// Both admissions are independently verified before corresponding native effects.
    fn live_factory(
        &self,
        client: ExecutionClient,
    ) -> impl FnOnce(JobContext, JobContext) -> Result<bool, ExportIssue> + Send + 'static {
        let source = self.source.alias();
        move |original_source, original_receiving| {
            let mut loan = source.loan(true)?;
            let operation = loan.operation()?;
            operation.execute(&client, &original_source, &original_receiving);
            Ok(operation.first.is_none())
        }
    }
    /// Submit both real admitted roles atomically onto one original ordinary worker.
    /// Rejection preserves every input and both preheld cleanup factories in this source.
    pub(in crate::production) fn submit_pair(
        &mut self,
        client: &ExecutionClient,
        source_scope: &Scope,
        source_spec: JobSpec,
        receiving_scope: &Scope,
        receiving_spec: JobSpec,
    ) -> Result<(), ExportIssue> {
        if self.delivery[..2]
            .iter()
            .any(|delivery| delivery.job.is_some() || delivery.completion.is_some())
            || source_scope.owner_id() != self.source.owner()
            || receiving_scope.owner_id() != self.source.owner()
        {
            return Err(ExportIssue::Started);
        }
        let source_permit = self.source_held.as_ref().ok_or(ExportIssue::Identity)?;
        let receiving_permit = self.receiving_held.as_ref().ok_or(ExportIssue::Identity)?;
        let request = ReportExportPairRequest::new(
            source_scope,
            source_spec,
            source_permit,
            receiving_scope,
            receiving_spec,
            receiving_permit,
        );
        match client.submit_report_export_pair(request, self.live_factory(client.clone())) {
            Ok((source, receiving)) => {
                self.delivery[0].job = Some(source);
                self.delivery[1].job = Some(receiving);
                Ok(())
            }
            Err(rejection) => Err(self.reject_admission(rejection.error)),
        }
    }
    /// Capture exact central terminal notices and completion guards independently for all jobs.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, ExportIssue> {
        let mut changed = false;
        let mut first = None;
        for delivery in &mut self.delivery {
            match delivery.capture(batch) {
                Ok(observed) => changed |= observed,
                Err(issue) => {
                    first.get_or_insert(issue);
                }
            }
        }
        if let Some(issue) = first {
            self.delivery_error.get_or_insert(issue);
            return Err(issue);
        }
        Ok(changed)
    }
    /// Reuse the fixed controller metadata after actual terminal adoption and disposal.
    /// A live terminal source guard rejects reuse; original causes/allocations cannot be replaced.
    pub(in crate::production) fn release_adopted(&mut self) -> Result<(), ExportIssue> {
        if (self.receipt().is_none() && !self.expected_receiving_failure())
            || self.delivery.iter().any(|delivery| delivery.completion.is_none())
            || !self.delivery[2..].iter().all(Delivery::succeeded)
            || self.source_held.is_some()
            || self.receiving_held.is_some()
        {
            return Err(ExportIssue::Busy);
        }
        self.source.release_adopted()?;
        self.delivery = std::array::from_fn(|_| Delivery::default());
        self.source_deadline = None;
        self.receiving_deadline = None;
        self.admission = None;
        self.delivery_error = None;
        self.cleanup_admitted = [false; 2];
        Ok(())
    }
    /// Cancel actual Main jobs; physical custody stays in this same controller source.
    pub(in crate::production) fn cancel(&self) {
        for delivery in &self.delivery[..2] {
            if let Some(job) = &delivery.job {
                job.cancel();
            }
        }
    }
    /// Success remains unavailable until both original Main and both actual cleanup deliveries.
    pub(in crate::production) fn receipt(&self) -> Option<ReportExportReceipt> {
        if self.delivery_error.is_some() || !self.delivery.iter().all(Delivery::succeeded) {
            return None;
        }
        self.source.inspect(|operation| {
            if !operation.source_closed
                || !operation.receiving_closed
                || operation.first.is_some()
                || operation.receiving_failure.is_some()
                || operation.source_failure.is_some()
            {
                return None;
            }
            operation.export.as_ref().and_then(ReportExportOperation::receipt)
        })?
    }
}

#[cfg(test)]
#[path = "service/tests.rs"]
mod tests;
