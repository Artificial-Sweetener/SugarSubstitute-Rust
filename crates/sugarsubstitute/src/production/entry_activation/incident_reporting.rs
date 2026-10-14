// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the genuine report effect, authenticated Core relay and guarded repository mutations.
#[path = "incident_reporting/action_result.rs"]
mod action_result;
#[path = "incident_reporting/admission.rs"]
mod admission;
#[path = "incident_reporting/delivery.rs"]
mod delivery;
#[path = "incident_reporting/desktop_failure.rs"]
mod desktop_failure;
pub(super) use desktop_failure::{DesktopFailureDisposition, PendingDesktopFailure};
#[path = "incident_reporting/custody.rs"]
mod custody;
#[path = "incident_reporting/retirement.rs"]
mod retirement;
#[path = "incident_reporting/session.rs"]
mod session;
use session::ReporterSession;
pub(in crate::production) use session::RetiredReporter;
#[path = "incident_reporting/export.rs"]
mod export;
#[path = "incident_reporting/recycling.rs"]
mod recycling;
#[path = "incident_reporting/restart.rs"]
mod restart;
pub(super) use restart::RestartAcceptance;
#[path = "incident_reporting/routing.rs"]
mod routing;
#[path = "incident_reporting/store.rs"]
mod store;
#[path = "incident_reporting/symbols.rs"]
mod symbols;
use symbols::SymbolReceiving;
#[cfg(test)]
#[path = "incident_reporting/tests.rs"]
mod tests;
use super::*;
use crate::production::{
    diagnostic_helper::reporting::*,
    incident_host::IncidentHost,
    reporter_role::{options, parent::ReporterLaunch, parent_source::*},
};
use std::{
    os::windows::ffi::OsStrExt,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use sugarsubstitute_application::incident::*;
use sugarsubstitute_contracts::{
    execution::*,
    incident::*,
    lifecycle::{ProcessKey, ShutdownId},
};
use sugarsubstitute_crash::diagnostic_client::*;
use sugarsubstitute_execution::{ExecutionClient, windows_diagnostic_pipe::*};
use sugarsubstitute_supervision::{
    LaunchEnvironment, RegistryAdmissionError, WindowsLaunchCommand, WindowsLaunchInput,
};

/// Keep the actual selected frame/token for the separately composed protected export owner.
pub(super) struct SelectedReportExport {
    pub(super) binding: ReportBinding,
    pub(super) context: IncidentRequestContext,
    pub(super) selection: ExportSelection,
    pub(super) authentication: ReportingAuthentication,
}
#[derive(Clone, Copy)]
enum StoreKind {
    View { previous: Option<IncidentRevision> },
    ExportView,
    Symbols,
    Attempt(ReportAttempt),
    Acknowledge(ReportCommand),
}
struct StoreWork {
    scope: IncidentOperationScope,
    binding: ReportBinding,
    kind: StoreKind,
    input: Option<ReportingInput>,
    installed: bool,
    submitted: bool,
    view_submitted: bool,
    cleanup_until: Option<Moment>,
    cleanup_submitted: bool,
    wire: Option<DiagnosticDeadline>,
    counter: delivery::Delivery<delivery::WireCounter>,
    observer: Option<DiagnosticReply>,
    adopted: bool,
    prepared: Option<PreparedReportView>,
    stream_plan: Option<ReporterViewPlan>,
    stream_until: Option<Moment>,
}
struct NativeRequest {
    authentication: ReportingAuthentication,
    reply: DiagnosticReply,
    context: IncidentRequestContext,
}
/// Original funded Reporter/controller custody remains until its actual independent retirements.
pub(super) struct IncidentReporting {
    relay: Option<ReporterRelay>,
    key: Option<DiagnosticKey>,
    binding: Option<ReportBinding>,
    locale: Option<sugarsubstitute_contracts::configuration::LocaleId>,
    original: Option<IncidentRequestContext>,
    report_budget: Option<IncidentOperationBudget>,
    queue_until: Option<Moment>,
    receiving_session: ReporterSession,
    reporter_startup: Option<sugarsubstitute_execution::RegisteredStartupSource>,
    launch_input: Option<delivery::LaunchDraft>,
    session: Option<DiagnosticPipeSession>,
    session_installed: bool,
    launched: bool,
    process: Option<ProcessKey>,
    wire_request: u64,
    store: Option<StoreWork>,
    request: Option<NativeRequest>,
    completion: Option<ReporterCompletion>,
    selected: Option<SelectedReportExport>,
    export_work: Option<export::ExportWork>,
    pending_control: Option<ReporterControlInput>,
    native_retired: bool,
    initial_loaded: bool,
    stopping: Option<ShutdownBudget>,
    family_retired: bool,
    first: Option<Box<dyn Error>>,
    dismiss_action: Option<ReportAction>,
    close_counter: Option<DiagnosticDeadline>,
    retained_effect: Option<IncidentEffect>,
    relay_until: Option<Moment>,
    relay_needed: bool,
    rejected_completion: Option<(ReceivedDiagnosticRequest, IssuedDiagnosticRequest)>,
    rejected_received: Option<(ReceivedDiagnosticRequest, DiagnosticAdmission)>,
    store_failure: Option<OwnedReportingFailure>,
    close_request: Option<DiagnosticRequestId>,
    reporter_failure: Option<OwnedReporterFailure>,
    symbols: Option<SymbolReceiving>,
}
const _: () = assert!(
    size_of::<IncidentReporting>() + ReporterSource::storage_bytes()
        < crate::production::reporter_role::parent::REPORTER_METADATA_BYTES as usize
);
impl IncidentReporting {
    pub(super) const fn new() -> Self {
        Self {
            relay: None,
            key: None,
            binding: None,
            locale: None,
            original: None,
            report_budget: None,
            queue_until: None,
            receiving_session: ReporterSession::new(),
            reporter_startup: None,
            launch_input: None,
            session: None,
            session_installed: false,
            launched: false,
            process: None,
            wire_request: 0,
            store: None,
            request: None,
            completion: None,
            selected: None,
            export_work: None,
            pending_control: None,
            native_retired: false,
            initial_loaded: false,
            stopping: None,
            family_retired: false,
            first: None,
            dismiss_action: None,
            close_counter: None,
            retained_effect: None,
            relay_until: None,
            relay_needed: false,
            rejected_completion: None,
            rejected_received: None,
            store_failure: None,
            close_request: None,
            reporter_failure: None,
            symbols: None,
        }
    }
    /// Feed the same immutable global batch to every actual finite owner exactly once.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        if let Some(symbols) = &mut self.symbols {
            symbols.capture(batch);
        }
        let preparation = self.receiving_session.capture(batch);
        let counter = self.store.as_mut().map(|work| work.counter.capture(batch)).transpose();
        let relay = self.relay.as_mut().map(|relay| relay.capture(batch)).transpose();
        preparation?;
        let _counter = counter?;
        let _relay = relay?;
        Ok(())
    }
    /// Retain exact first caller/provider cause with all original inputs and native owners.
    pub(super) fn progress(
        &mut self,
        host: &mut IncidentHost,
        records: &mut [Option<RetainedRecord>; 64],
        executable: &Path,
        working: &Path,
        resources: &mut EntryResources<'_>,
        closing: bool,
    ) -> Result<()> {
        let result = self.advance(host, records, executable, working, resources, closing);
        if let Err(cause) = result
            && self.first.is_none()
        {
            self.first = Some(cause);
            return Err("Actual report receiving failed with retained original custody".into());
        }
        Ok(())
    }
    fn advance(
        &mut self,
        host: &mut IncidentHost,
        records: &mut [Option<RetainedRecord>; 64],
        executable: &Path,
        working: &Path,
        resources: &mut EntryResources<'_>,
        closing: bool,
    ) -> Result<()> {
        if closing
            || self.first.is_some()
            || self.native_retired
                && self.completion.is_none()
                && self.store.is_none()
                && self.export_work.is_none()
        {
            return self.retire(host, resources);
        }
        if self.binding.is_none() {
            self.admit(host, records, resources)?;
        }
        if self.binding.is_none() {
            return Ok(());
        }
        self.prepare_launch(executable, working, resources)?;
        if !self.launched {
            return Ok(());
        }
        self.capture_output(host, resources)?;
        self.advance_store(host, resources)?;
        self.consume_effect(host, resources)?;
        self.advance_export(host, resources)?;
        if self.selected.is_some()
            || self.export_work.as_ref().is_some_and(|work| work.awaiting_result())
        {
            return Ok(());
        }
        self.drive_relay(resources)?;
        Ok(())
    }
    fn relay(&self) -> Result<&ReporterRelay> {
        self.relay.as_ref().ok_or_else(|| "Actual Reporter relay missing".into())
    }
    /// Borrow retained failure without replacing original guarded worker/native causes.
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.store_failure
            .as_ref()
            .map(|cause| cause as _)
            .or_else(|| self.reporter_failure.as_ref().map(|cause| cause as _))
            .or_else(|| self.symbols.as_ref().and_then(SymbolReceiving::failure))
            .or_else(|| self.receiving_session.failure())
            .or(self.first.as_deref())
    }
    /// An actual ended window still retains process, pipe and source custody until checked cleanup.
    pub(super) fn settled(&self) -> bool {
        self.symbols.as_ref().is_none_or(SymbolReceiving::settled)
            && self.receiving_session.delivered()
            && (self.relay.is_none()
                || self.binding.is_none()
                    && self.relay.as_ref().is_some_and(ReporterRelay::idle_funded)
                || self.family_retired && self.store.is_none() && self.export_work.is_none())
    }
    /// Earliest retained finite cutoff feeds the existing native wake; it creates no timer.
    pub(super) fn next_deadline(&self) -> Option<Moment> {
        self.stopping
            .map(|budget| budget.settle_until)
            .or_else(|| {
                self.store
                    .as_ref()
                    .map(|work| work.cleanup_until.unwrap_or(work.scope.original().until()))
            })
            .or_else(|| self.export_work.as_ref().and_then(|work| work.until()))
            .or(self.queue_until)
    }
    fn allocate_wire(&mut self) -> Result<DiagnosticRequestId> {
        self.wire_request =
            self.wire_request.checked_add(1).ok_or("Reporter wire identity exhausted")?;
        Ok(DiagnosticRequestId::new(self.wire_request)?)
    }
}
