// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one immutable effect and one whole custody phase through actual export completion.
use super::*;
use crate::production::diagnostic_helper::exporting::{
    ExportCompletionCustody, ExportCompletionParts, ExportInput, ExportIssue, ExportService,
    ExportTerminal,
};
#[path = "export/control.rs"]
mod control;
#[path = "export/retire.rs"]
mod retire;
#[path = "export/scopes.rs"]
mod scopes;
/// Mutually exclusive original owners share bounded metadata without splitting the prepared view.
enum ExportMaterial {
    Selected {
        original: SelectedReportExport,
        view: Option<PreparedReportView>,
    },
    Input(ExportInput),
    Bound,
    Completion {
        request: DiagnosticRequestId,
        view: Option<PreparedReportView>,
        custody: Option<ExportCompletionCustody>,
    },
    Retiring(ExportTerminal),
    Returned {
        view: Option<PreparedReportView>,
        custody: Option<ExportCompletionCustody>,
    },
}
/// Actual scopes, immutable job identities and whole custody remain within original metadata.
pub(super) struct ExportWork {
    transaction: Option<DiagnosticTransaction>,
    material: ExportMaterial,
    jobs: scopes::ExportJobs,
    bound: bool,
    source_held: bool,
    receiving_held: bool,
    submitted: bool,
    rejected_admission: bool,
    wake_until: Option<Moment>,
}
impl ExportWork {
    /// Pause the genuine incoming loan until the actual export result is adopted.
    pub(super) fn awaiting_result(&self) -> bool {
        !matches!(self.material, ExportMaterial::Completion { .. })
    }
    /// Borrow an already fixed action/cleanup cutoff without renewal.
    pub(super) fn until(&self) -> Option<Moment> {
        self.wake_until
    }
}
impl IncidentReporting {
    /// Borrow the same funded immutable effect; copied metadata grants no native authority.
    fn original_export(&self) -> Result<(IncidentOperationScope, ReportBinding, ExportSelection)> {
        let Some(IncidentEffect::Export { scope, binding, selection }) =
            self.retained_effect.as_ref()
        else {
            return Err("Original export effect missing".into());
        };
        Ok((*scope, *binding, *selection))
    }
    pub(super) fn begin_export(
        &mut self,
        host: &mut IncidentHost,
        scope: IncidentOperationScope,
        binding: ReportBinding,
        selection: ExportSelection,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        if self.export_work.is_some() || self.store.is_some() {
            return Err("Actual export already retained".into());
        }
        if self.original_export()? != (scope, binding, selection) {
            return Err("Original export effect changed".into());
        }
        let selected = self.selected.as_ref().ok_or("Authenticated selected export missing")?;
        if selected.binding != binding
            || selected.selection != selection
            || scope.incident() != binding.incident()
            || scope.expected_revision() != binding.revision()
            || scope.original() != selected.context.budget()
            || self.binding != Some(binding)
        {
            return Err("Original selected export correlation changed".into());
        }
        let jobs =
            scopes::ExportJobs::new(resources.client, resources.scopes, scope.original().until())?;
        let read = host.admit_report_view(binding, selected.context)?;
        let at = selected.context.at();
        self.export_work = Some(ExportWork {
            transaction: None,
            material: ExportMaterial::Selected {
                original: self.selected.take().ok_or("Original selected frame missing")?,
                view: None,
            },
            jobs,
            bound: false,
            source_held: false,
            receiving_held: false,
            submitted: false,
            rejected_admission: false,
            wake_until: Some(scope.original().until()),
        });
        self.begin_store(host, read, binding, StoreKind::ExportView, at)
    }
    /// Install the whole exact view after actual repository writer retirement.
    pub(super) fn install_export_view(&mut self, view: PreparedReportView) -> Result<()> {
        let (_, binding, _) = self.original_export()?;
        let work = self.export_work.as_mut().ok_or("Original export view owner missing")?;
        let ExportMaterial::Selected { view: slot, .. } = &mut work.material else {
            return Err("Original export view phase changed".into());
        };
        if slot.is_some() {
            return Err("Actual export view already retained".into());
        }
        *slot = Some(view);
        if slot.as_ref().is_none_or(|view| view.view().binding() != binding) {
            return Err("Actual export view changed with whole owner retained".into());
        }
        Ok(())
    }
    /// The central pair executes both genuine roles on the same original ordinary worker.
    pub(super) fn advance_export(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<()> {
        if self.export_work.is_none() {
            return Ok(());
        }
        let (scope, binding, _) = self.original_export()?;
        let work = self.export_work.as_mut().ok_or("Original export worker missing")?;
        if self.store.is_some() || !work.awaiting_result() {
            return Ok(());
        }
        if !work.submitted && resources.client.now()? >= scope.original().until() {
            return Err("Original selected export deadline expired with inputs retained".into());
        }
        if let ExportMaterial::Selected { view, .. } = &work.material {
            if view.is_none() {
                return Ok(());
            }
            let protected = host.namespace()?.committed_read_request(scope.operation())?;
            let prior = std::mem::replace(&mut work.material, ExportMaterial::Bound);
            if let ExportMaterial::Selected { original, view } = prior {
                work.material = ExportMaterial::Input(ExportInput {
                    protected: Some(protected),
                    view,
                    received: original.authentication.received,
                    issued: original.authentication.issued,
                });
            } else {
                work.material = prior;
                return Err("Original export selection phase changed".into());
            }
        }
        let service = host.export_service()?;
        if work.rejected_admission {
            if !service.retire_unsubmitted()? {
                return Ok(());
            }
            host.adopt_export_admission_failure(scope, binding)?;
            let input = host
                .export_service()?
                .take_unsubmitted_input()?
                .ok_or("Original rejected export input missing after actual cancellation")?;
            transactions.retired(scope.operation(), &mut work.transaction)?;
            // Genuine unpaired cancellation deliveries precede the failed action result.
            // No native Main ran, and this request never creates a second admission attempt.
            return self.complete_rejected_export_control(scope, binding, input);
        }
        if !work.bound
            && !transactions.admit(
                DiagnosticTransactionRole::Export,
                scope.operation(),
                &mut work.transaction,
            )?
        {
            return Ok(());
        }
        work.jobs.prepare(resources.jobs)?;
        if !work.bound {
            let prior = std::mem::replace(&mut work.material, ExportMaterial::Bound);
            let ExportMaterial::Input(input) = prior else {
                work.material = prior;
                return Err("Original export input phase changed".into());
            };
            let mut candidate = Some(input);
            let result = service.bind(&mut candidate);
            work.bound = candidate.is_none();
            if let Some(input) = candidate {
                work.material = ExportMaterial::Input(input);
            }
            result?;
        }
        if !work.source_held {
            match service.reserve_source_cleanup(
                resources.client,
                &work.jobs.source_cleanup,
                work.jobs.source_cleanup_spec()?,
                &work.jobs.source,
                work.jobs.source_spec()?,
            ) {
                Ok(()) => work.source_held = true,
                Err(ExportIssue::Admission) => {
                    work.rejected_admission = true;
                    if let Some(cause) = service.admission_failure() {
                        tracing::warn!(
                            operation = "report_export_admission_rejected",
                            cause = ?crate::production::entry_runner::FailureDisplay(cause),
                            "Original export overload retained until actual unpaired cancellation"
                        );
                    }
                    return Ok(());
                }
                Err(ExportIssue::Rejected) => {
                    Self::observe_export_failure(service);
                    return Err(ExportIssue::Rejected.into());
                }
                Err(cause) => return Err(cause.into()),
            }
        }
        if !work.receiving_held {
            match service.reserve_receiving_cleanup(
                resources.client,
                &work.jobs.receiving_cleanup,
                work.jobs.receiving_cleanup_spec()?,
                &work.jobs.receiving,
                work.jobs.receiving_spec()?,
            ) {
                Ok(()) => work.receiving_held = true,
                Err(ExportIssue::Admission) => {
                    work.rejected_admission = true;
                    if let Some(cause) = service.admission_failure() {
                        tracing::warn!(
                            operation = "report_export_admission_rejected",
                            cause = ?crate::production::entry_runner::FailureDisplay(cause),
                            "Original export overload retained until actual unpaired cancellation"
                        );
                    }
                    return Ok(());
                }
                Err(ExportIssue::Rejected) => {
                    Self::observe_export_failure(service);
                    return Err(ExportIssue::Rejected.into());
                }
                Err(cause) => return Err(cause.into()),
            }
        }
        if !work.submitted {
            match service.submit_pair(
                resources.client,
                &work.jobs.source,
                work.jobs.source_spec()?,
                &work.jobs.receiving,
                work.jobs.receiving_spec()?,
            ) {
                Ok(()) => work.submitted = true,
                Err(ExportIssue::Admission) => {
                    work.rejected_admission = true;
                    if let Some(cause) = service.admission_failure() {
                        tracing::warn!(
                            operation = "report_export_admission_rejected",
                            cause = ?crate::production::entry_runner::FailureDisplay(cause),
                            "Original export overload retained until actual unpaired cancellation"
                        );
                    }
                    return Ok(());
                }
                Err(ExportIssue::Rejected) => {
                    Self::observe_export_failure(service);
                    return Err(ExportIssue::Rejected.into());
                }
                Err(cause) => return Err(cause.into()),
            }
        }
        match service.advance_cleanup(resources.client) {
            Ok(_) => {}
            Err(ExportIssue::Admission) => return Ok(()),
            Err(ExportIssue::Rejected) => {
                Self::observe_export_failure(service);
                return Err(ExportIssue::Rejected.into());
            }
            Err(cause) => return Err(cause.into()),
        }
        work.wake_until = service.next_deadline();
        if service.inspect_execution_failure(|_| ()).is_some() || service.cleanup_failed() {
            Self::observe_export_failure(service);
            return Err("Original export worker failure retained for checked retirement".into());
        }
        if !service.checked_terminal() {
            return Ok(());
        }
        transactions.retired(scope.operation(), &mut work.transaction)?;
        self.complete_export_control(host, scope, binding)
    }
    /// Observe original guarded causes while the service retains their exact owned lifetimes.
    fn observe_export_failure(service: &ExportService) {
        if service.inspect_failure(|cause, custody| {
            if let Some(cause) = cause {
                tracing::error!(
                    operation = "report_export_retained_failure",
                    error = ?crate::production::entry_runner::FailureDisplay(cause),
                    custody = ?custody,
                    "Original export failure remains retained through checked retirement"
                );
            } else {
                tracing::error!(
                    operation = "report_export_retained_outcome",
                    custody = ?custody,
                    "Original unexpected export outcome remains retained through checked retirement"
                );
            }
        }).is_none() {
            tracing::error!(
                operation = "report_export_retained_source_busy",
                "Original export source remains retained by its active worker"
            );
        }
    }
    /// Release the whole view and source only after actual matched accepted Core completion.
    pub(super) fn accept_export_completion(
        &mut self,
        host: &mut IncidentHost,
        key: DiagnosticKey,
        request: DiagnosticRequestId,
    ) -> Result<bool> {
        let Some(work) = self.export_work.as_ref() else { return Ok(false) };
        if self.key != Some(key)
            || !matches!(&work.material, ExportMaterial::Completion { request: original, .. } if *original == request)
        {
            return Err("Original export Core completion changed".into());
        }
        self.relay_until = Some(work.jobs.until());
        let submitted = work.submitted;
        drop(self.export_work.take());
        if submitted {
            host.export_service()?.release_adopted()?;
        } else {
            host.export_service()?.release_unsubmitted()?;
        }
        self.retained_effect = None;
        Ok(true)
    }
}
