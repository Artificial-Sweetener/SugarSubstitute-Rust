// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the genuine report effect, authenticated Core relay and guarded repository mutations.
#[path = "incident_reporting/action_result.rs"]
mod action_result;
#[path = "incident_reporting/admission.rs"]
mod admission;
#[path = "incident_reporting/deadlines.rs"]
mod deadlines;
#[path = "incident_reporting/delivery.rs"]
mod delivery;
#[path = "incident_reporting/desktop_failure.rs"]
mod desktop_failure;
#[path = "incident_reporting/fresh_restart.rs"]
mod fresh_restart;
#[path = "incident_reporting/restart_guard.rs"]
mod restart_guard;
pub(super) use desktop_failure::{DesktopFailureDisposition, PendingDesktopFailure};
pub(super) use fresh_restart::DeferredFreshRestart;
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
pub(super) use restart::{RestartAcceptance, RestartTarget};
#[path = "incident_reporting/receiving_failure.rs"]
mod receiving_failure;
#[path = "incident_reporting/routing.rs"]
mod routing;
#[path = "incident_reporting/source_observation.rs"]
mod source_observation;
#[path = "incident_reporting/store.rs"]
mod store;
#[path = "incident_reporting/symbols.rs"]
mod symbols;
use symbols::SymbolReceiving;
#[cfg(test)]
#[path = "incident_reporting/tests.rs"]
mod tests;
use super::incident_flow::transactions::{
    DiagnosticTransaction, DiagnosticTransactionRole, DiagnosticTransactions,
};
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
/// Closed native-thread transitions identify the actual rejected receiving operation.
#[derive(Clone, Copy, Debug)]
enum ReportTransition {
    Retire,
    Admit,
    PrepareLaunch,
    CaptureOutput,
    AdvanceStore,
    ConsumeEffect,
    AdvanceExport,
    DriveRelay,
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
    transaction: Option<DiagnosticTransaction>,
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
/// This native turn's actual Root admission facts; these grant no native privilege.
pub(super) struct ReportAdmission {
    closing: bool,
    restart: ReportRestartAvailability,
    fresh_supervisor: bool,
}
impl ReportAdmission {
    /// Retain this turn's current Root capture decision and actual closing state.
    /// This inert value creates no source or native privilege.
    pub(super) const fn new(
        closing: bool,
        restart: ReportRestartAvailability,
        fresh_supervisor: bool,
    ) -> Self {
        Self { closing, restart, fresh_supervisor }
    }
}
/// Original funded Reporter/controller custody remains until its actual independent retirements.
pub(super) struct IncidentReporting {
    fresh_supervisor: bool,
    deferred_restart: Option<DeferredFreshRestart>,
    restart: ReportRestartAvailability,
    relay: Option<ReporterRelay>,
    key: Option<DiagnosticKey>,
    binding: Option<ReportBinding>,
    locale: Option<sugarsubstitute_contracts::configuration::LocaleId>,
    original: Option<IncidentRequestContext>,
    report_budget: Option<IncidentOperationBudget>,
    queue_until: Option<Moment>,
    receiving_session: ReporterSession,
    reporter_startup: Option<sugarsubstitute_execution::RegisteredStartupSource>,
    recovery_source: Option<recovery::CurrentReportSource>,
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
    initial_view_admitted: bool,
    authenticated_peer_observed: Option<(DiagnosticKey, ProcessKey)>,
    stopping: Option<ShutdownBudget>,
    family_retired: bool,
    first: Option<Box<dyn Error>>,
    secondary_receiving_failure: Option<receiving_failure::SecondaryFailure>,
    omitted_receiving_failures: u64,
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
            fresh_supervisor: false,
            deferred_restart: None,
            restart: ReportRestartAvailability::Unavailable,
            relay: None,
            key: None,
            binding: None,
            locale: None,
            original: None,
            report_budget: None,
            queue_until: None,
            receiving_session: ReporterSession::new(),
            reporter_startup: None,
            recovery_source: None,
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
            initial_view_admitted: false,
            authenticated_peer_observed: None,
            stopping: None,
            family_retired: false,
            first: None,
            secondary_receiving_failure: None,
            omitted_receiving_failures: 0,
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn progress(
        &mut self,
        host: &mut IncidentHost,
        records: &mut [Option<RetainedRecord>; 64],
        executable: &Path,
        working: &Path,
        resources: &mut EntryResources<'_>,
        admission: ReportAdmission,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<()> {
        self.restart = admission.restart;
        self.fresh_supervisor = admission.fresh_supervisor;
        let mut transition = ReportTransition::Retire;
        let result = self.advance(
            host,
            records,
            executable,
            working,
            resources,
            admission.closing,
            &mut transition,
            transactions,
        );
        if let Err(cause) = result {
            return self.retain_transition_failure(transition, cause);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn advance(
        &mut self,
        host: &mut IncidentHost,
        records: &mut [Option<RetainedRecord>; 64],
        executable: &Path,
        working: &Path,
        resources: &mut EntryResources<'_>,
        closing: bool,
        transition: &mut ReportTransition,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<()> {
        if closing
            || self.first.is_some()
            || self.native_retired
                && self.completion.is_none()
                && self.store.is_none()
                && self.export_work.is_none()
        {
            *transition = ReportTransition::Retire;
            return self.retire(host, resources, transactions);
        }
        if self.binding.is_none() {
            *transition = ReportTransition::Admit;
            self.admit(host, records, resources, transactions)?;
        }
        if self.binding.is_none() {
            return Ok(());
        }
        *transition = ReportTransition::PrepareLaunch;
        self.prepare_launch(executable, working, resources)?;
        if !self.launched {
            return Ok(());
        }
        *transition = ReportTransition::CaptureOutput;
        self.capture_output(host, resources)?;
        *transition = ReportTransition::AdvanceStore;
        self.advance_store(host, resources, transactions)?;
        *transition = ReportTransition::ConsumeEffect;
        self.consume_effect(host, resources)?;
        *transition = ReportTransition::AdvanceExport;
        self.advance_export(host, resources, transactions)?;
        if self.selected.is_some()
            || self.export_work.as_ref().is_some_and(|work| work.awaiting_result())
        {
            return Ok(());
        }
        *transition = ReportTransition::DriveRelay;
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
    fn allocate_wire(&mut self) -> Result<DiagnosticRequestId> {
        self.wire_request =
            self.wire_request.checked_add(1).ok_or("Reporter wire identity exhausted")?;
        Ok(DiagnosticRequestId::new(self.wire_request)?)
    }
}

#[path = "incident_reporting/recovery.rs"]
mod recovery;
