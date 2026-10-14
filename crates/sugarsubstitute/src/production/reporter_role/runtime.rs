// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the real Reporter role using one execution owner, authentic pipe and native surface.
use super::{
    client::ReporterClient, options::ReporterRole, output::*, service::ViewService, view::*,
};
use crate::job_sequence::JobSequence;
use std::{
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::diagnostic_client::*;
use sugarsubstitute_desktop::report::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
const STORAGE_BYTES: u64 = 6 * 1024 * 1024;
const METADATA_BYTES: u64 = 64 * 1024;
struct Funding {
    view: ViewService,
    ui: MemoryLease,
    _metadata: MemoryClaim,
}
pub(super) struct ReporterSession {
    channel: ReporterClient,
    locale: sugarsubstitute_contracts::configuration::LocaleId,
    execution: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    native: Option<Scope>,
    finite: Option<Scope>,
    jobs: JobSequence,
    constructor: Option<OwnedJob<Funding, MemoryClaimError>>,
    construction: Option<JobCompletion<Funding, MemoryClaimError>>,
    funding: Option<Funding>,
    received: Option<super::client::NativeInput>,
    view: Option<NativeReportView>,
    current: Option<ReportBinding>,
    outgoing: [Option<OutputInput>; 8],
    encoding: Option<OwnedJob<EncodedOutput, OutputFailure>>,
    encoded: Option<JobCompletion<EncodedOutput, OutputFailure>>,
    written: Option<sugarsubstitute_execution::diagnostic_pipe::LifecyclePipeOperationId>,
    next_request: u64,
    pending: Option<(ReportRequestId, ReportCommand, DiagnosticRequestId)>,
    rejected_control: Option<ControlResult>,
    update: Option<ReportSurfaceUpdate>,
    original: Option<Moment>,
    deadline: Option<Instant>,
    started: bool,
    closing: bool,
    native_retired: bool,
    execution_stopping: bool,
    awaiting_surface: bool,
    shutdown_until: Option<Moment>,
    view_failed: bool,
    output_failed: bool,
}
const _: () = assert!(
    size_of::<ReporterSession>()
        + size_of::<Funding>()
        + size_of::<super::service::OwnedViewAdmissionFailure>()
        + size_of::<output::OwnedOutputAdmissionFailure>()
        + 4 * size_of::<usize>()
        < METADATA_BYTES as usize
);
pub(in crate::production) fn run(role: ReporterRole) -> Result<(), ReportSurfaceError> {
    let session = ReporterSession::new(role);
    let receipt = run_prepared_report_surface(session)?;
    tracing::info!(
        operation = "reporter_retired",
        session = receipt.session().get(),
        presented = receipt.presented(),
        "Native Reporter retired"
    );
    Ok(())
}
impl ReporterSession {
    fn new(role: ReporterRole) -> Self {
        Self {
            channel: ReporterClient::new(role.bootstrap, role.original),
            locale: role.locale,
            execution: None,
            client: None,
            native: None,
            finite: None,
            jobs: JobSequence::new(),
            constructor: None,
            construction: None,
            funding: None,
            received: None,
            view: None,
            current: None,
            outgoing: std::array::from_fn(|_| None),
            encoding: None,
            encoded: None,
            written: None,
            next_request: 0,
            pending: None,
            rejected_control: None,
            update: None,
            original: None,
            deadline: None,
            started: false,
            closing: false,
            native_retired: false,
            execution_stopping: false,
            awaiting_surface: false,
            shutdown_until: None,
            view_failed: false,
            output_failed: false,
        }
    }
    fn rejected(operation: &'static str) -> ReportSurfaceError {
        ReportSurfaceError::new(
            operation,
            std::io::Error::other("Reporter owner transition rejected"),
        )
    }
    fn consume(&mut self, maximum: NonZeroUsize) -> Result<(), ReportSurfaceError> {
        let Some(client) = self.client.as_ref().cloned() else {
            return Ok(());
        };
        let batch = client
            .drain_completions(maximum)
            .map_err(|cause| ReportSurfaceError::new("reporter_completions", cause))?;
        self.channel.poll().map_err(|cause| {
            tracing::error!(
                operation = "reporter_transport",
                original_retained = self.channel.failure().is_some(),
                "Reporter transport failed"
            );
            ReportSurfaceError::new("reporter_transport", cause)
        })?;
        if let Some(job) = &mut self.constructor
            && let Some(done) = job.try_take_completion()
        {
            self.construction = Some(done);
            self.constructor = None;
        }
        if self.funding.is_none()
            && let Some(done) = self.construction.take()
        {
            match done.outcome {
                JobOutcome::Success(funding) => self.funding = Some(funding),
                JobOutcome::Failure(cause) => {
                    return Err(ReportSurfaceError::new("reporter_funding", cause));
                }
                other => {
                    self.construction = Some(JobCompletion { outcome: other, ..done });
                    return Err(Self::rejected("reporter_funding_stopped"));
                }
            }
        }
        if let Some(funding) = &mut self.funding {
            funding.view.capture(&batch).map_err(|_| Self::rejected("reporter_view_terminal"))?;
            let prepared = if self.view_failed {
                None
            } else {
                match funding.view.take_prepared() {
                    Ok(prepared) => prepared,
                    Err(_) => {
                        tracing::error!(
                            operation = "reporter_view_terminal",
                            original_panic_retained = funding.view.panic_cause().is_some(),
                            "Reporter view worker stopped"
                        );
                        self.view_failed = true;
                        return Err(match funding.view.take_failure() {
                            Some(cause) => ReportSurfaceError::new("reporter_view_failed", cause),
                            None => Self::rejected("reporter_view_failed"),
                        });
                    }
                }
            };
            if let Some(prepared) = prepared {
                match prepared {
                    PreparedView::Partial => {}
                    PreparedView::Initial(view) => {
                        let view = view.into_native()?;
                        self.current = Some(view.binding());
                        self.view = Some(view);
                    }
                    PreparedView::Replacement(view) => {
                        let old = self
                            .current
                            .ok_or_else(|| Self::rejected("reporter_replace_before_view"))?;
                        let view = view.into_native()?;
                        let current = view.binding();
                        self.current = Some(current);
                        self.update = Some(ReportSurfaceUpdate::ReplaceView { old, current, view });
                    }
                    PreparedView::Control(control) => self.control(control)?,
                }
                self.channel
                    .continue_read()
                    .map_err(|cause| ReportSurfaceError::new("reporter_read", cause))?;
            }
        }
        if !self.closing && self.received.is_none() {
            self.received = self.channel.take_native_input();
        }
        if self.received.is_some()
            && let (Some(funding), Some(finite)) = (&mut self.funding, &self.finite)
        {
            let until = match funding.view.input_until() {
                Some(original) => original,
                None if self.current.is_none() => self
                    .original
                    .ok_or_else(|| Self::rejected("reporter_preparation_deadline_missing"))?,
                None => client
                    .now()
                    .map_err(|cause| ReportSurfaceError::new("reporter_input_clock", cause))?
                    .checked_add(Duration::from_secs(30))
                    .map_err(|cause| ReportSurfaceError::new("reporter_input_deadline", cause))?,
            };
            if funding.view.submit(&client, finite, &self.jobs, until, &mut self.received).is_err()
            {
                self.view_failed = true;
                return Err(match funding.view.take_admission_failure() {
                    Some(cause) => ReportSurfaceError::new("reporter_view_admission", cause),
                    None => Self::rejected("reporter_view_admission"),
                });
            }
        }
        self.consume_output()?;
        Ok(())
    }
    fn enqueue(
        &mut self,
        binding: ReportBinding,
        intent: Intent,
    ) -> Result<DiagnosticRequestId, ReportSurfaceError> {
        if self.outgoing.iter().all(Option::is_some) {
            return Err(Self::rejected("reporter_output_capacity"));
        }
        let client =
            self.client.as_ref().ok_or_else(|| Self::rejected("reporter_execution_missing"))?;
        let until = client
            .now()
            .map_err(|cause| ReportSurfaceError::new("reporter_action_clock", cause))?
            .checked_add(Duration::from_secs(15))
            .map_err(|cause| ReportSurfaceError::new("reporter_action_deadline", cause))?;
        let counter = client
            .encode_lifecycle_deadline(until)
            .map_err(|cause| ReportSurfaceError::new("reporter_action_counter", cause))?;
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or_else(|| Self::rejected("reporter_request_exhausted"))?;
        let request = DiagnosticRequestId::new(self.next_request)
            .map_err(|cause| ReportSurfaceError::new("reporter_request", cause))?;
        let key =
            self.channel.key().ok_or_else(|| Self::rejected("reporter_unauthenticated_output"))?;
        let slot = self
            .outgoing
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or_else(|| Self::rejected("reporter_output_capacity"))?;
        *slot = Some(OutputInput {
            key,
            request,
            binding,
            until,
            counter: DiagnosticDeadline::new(counter.ticks(), counter.frequency())
                .map_err(|cause| ReportSurfaceError::new("reporter_counter_shape", cause))?,
            buffer: None,
            intent,
        });
        Ok(request)
    }
}

#[path = "runtime/lifetime.rs"]
mod lifetime;
#[path = "runtime/output.rs"]
mod output;

#[path = "runtime/control.rs"]
mod control;
