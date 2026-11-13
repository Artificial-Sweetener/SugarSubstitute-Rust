// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Parse, encode and complete genuine router custody exclusively on finite Core admissions.
use super::*;
use sha2::{Digest, Sha256};
pub(in crate::production) const REPORTER_CORE_BYTES: u64 = 64 * 1024;
/// One finite worker either inspected the genuine source or found its loan temporarily held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelayTurn {
    /// The worker borrowed and checked the original source; its actual output may still be pending.
    Advanced,
    /// Another genuine caller holds the source loan; no source state or native authority was inspected.
    Deferred,
}
impl ReporterSource {
    /// Execute one bounded relay turn; only unavailable loan acquisition defers without source mutation.
    /// Semantic errors after acquisition retain their actual failure and native/session custody.
    pub(super) fn process(
        &self,
        context: &JobContext,
        read_now: impl FnOnce() -> Result<Moment, ExecutionError>,
        encode_until: impl FnOnce(Moment) -> Result<CounterDeadline, NativeObservationError>,
    ) -> Result<RelayTurn, ReporterSourceIssue> {
        if context.owner_id() != self.shared.owner
            || context.spec().reservation_bytes != REPORTER_CORE_BYTES
            || context.spec().operation != Operation::NativeLifecycle
            || context.spec().class != JobClass::InfrastructureLifecycle
            || context.deadline().is_none()
        {
            return Err(ReporterSourceIssue::Identity);
        }
        let mut loan = match self.loan() {
            Ok(loan) => loan,
            Err(ReporterSourceIssue::Busy) => return Ok(RelayTurn::Deferred),
            Err(cause) => return Err(cause),
        };
        let state = loan.state()?;
        if state.failure_transferred {
            return Err(ReporterSourceIssue::Parent);
        }
        if let Some(first) = state.first {
            return Err(first);
        }
        let now = read_now().map_err(|cause| {
            state.time_cause.get_or_insert(time::ReporterTimeCause::Clock(cause));
            state.first.get_or_insert(ReporterSourceIssue::Clock);
            ReporterSourceIssue::Clock
        })?;
        if let Err(cause) =
            state.check(context, now).and_then(|()| state.step(context, now, encode_until))
        {
            state.first.get_or_insert(cause);
            return Err(cause);
        }
        Ok(RelayTurn::Advanced)
    }
    /// Caller requests another read only after disposing prior received custody at its owner.
    pub(in crate::production) fn resume_read(&self) -> Result<(), ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        if state.output.is_some() || state.completion.is_some() || state.first.is_some() {
            return Err(ReporterSourceIssue::Busy);
        }
        state.read_after_complete = true;
        Ok(())
    }
    /// Fixed earliest active transfer cutoff for the next central finite admission.
    pub(super) fn cutoff(&self, until: Moment) -> Result<Moment, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        let view = state.views.iter().flatten().map(stream::Stream::until).min();
        Ok(view
            .into_iter()
            .chain(state.control.as_ref().map(|input| input.until))
            .fold(until, Moment::min))
    }
    /// Controller calls this only after matching the actual job's central terminal and completion.
    pub(super) fn take_output(&self) -> Result<Option<ReporterOutput>, ReporterSourceIssue> {
        let mut loan = self.loan()?;
        let state = loan.state()?;
        for slot in &mut state.views {
            if slot.as_ref().is_some_and(stream::Stream::delivered) {
                *slot = None;
            }
        }
        Ok(state.output.take())
    }
}
impl State {
    fn check(&mut self, context: &JobContext, now: Moment) -> Result<(), ReporterSourceIssue> {
        if context.cancellation_requested() {
            return Err(ReporterSourceIssue::Cancelled);
        }
        let until = context.deadline().ok_or(ReporterSourceIssue::Identity)?;
        if self.views.iter().flatten().any(|view| until > view.until())
            || self.control.as_ref().is_some_and(|input| until > input.until)
        {
            return Err(ReporterSourceIssue::Identity);
        }
        if self.core_scope.is_some_and(|scope| scope != context.scope_id()) {
            return Err(ReporterSourceIssue::Identity);
        }
        if now >= until || self.last_observed.is_some_and(|before| now < before) {
            return Err(ReporterSourceIssue::Deadline);
        }
        self.core_scope = Some(context.scope_id());
        self.last_observed = Some(now);
        for view in self.views.iter_mut().flatten() {
            view.latch(until)?;
        }
        Ok(())
    }
    fn step(
        &mut self,
        context: &JobContext,
        now: Moment,
        encode_until: impl FnOnce(Moment) -> Result<CounterDeadline, NativeObservationError>,
    ) -> Result<(), ReporterSourceIssue> {
        self.poll()?;
        if self.output.is_some() {
            return Ok(());
        }
        if self.control_written {
            let operation = self.control_write.take().ok_or(ReporterSourceIssue::Identity)?;
            self.control = None;
            self.control_written = false;
            self.output = Some(ReporterOutput::ControlWritten(operation));
            return Ok(());
        }
        if let Some(input) = &mut self.control {
            input.prepare_counter(encode_until).map_err(|cause| {
                self.time_cause.get_or_insert(cause);
                ReporterSourceIssue::Clock
            })?;
        }
        let parent = self.parent.as_mut().ok_or(ReporterSourceIssue::Busy)?;
        if let Some(input) = self.completion.as_ref() {
            let accepted = parent.complete(&input.issued, input.reply).is_ok();
            let input = self.completion.take().ok_or(ReporterSourceIssue::Identity)?;
            self.output = Some(ReporterOutput::Completed {
                received: input.received,
                issued: input.issued,
                accepted,
            });
            return if accepted { Ok(()) } else { Err(ReporterSourceIssue::Parent) };
        }
        if self.read_after_complete {
            parent.continue_read().map_err(|_| ReporterSourceIssue::Parent)?;
            self.read_after_complete = false;
        }
        if !parent.authenticated() {
            return Ok(());
        }
        if let Some((received, admission)) =
            parent.receive().map_err(|_| ReporterSourceIssue::Parent)?
        {
            let frame = match received.decode() {
                Ok(frame) => frame,
                Err(cause) => {
                    self.wire.get_or_insert(cause);
                    self.rejected = Some((received, admission));
                    return Err(ReporterSourceIssue::Wire);
                }
            };
            let observation = match observation(frame) {
                Ok(observation) => observation,
                Err(cause) => {
                    self.wire.get_or_insert(cause);
                    self.rejected = Some((received, admission));
                    return Err(ReporterSourceIssue::Wire);
                }
            };
            let request_context = match time::request(
                now,
                context.deadline().ok_or(ReporterSourceIssue::Identity)?,
            ) {
                Ok(receipt) => receipt,
                Err(cause) => {
                    self.time_cause.get_or_insert(cause);
                    self.rejected = Some((received, admission));
                    return Err(ReporterSourceIssue::Clock);
                }
            };
            for view in self.views.iter_mut().flatten() {
                view.presentation(frame);
            }
            self.output = Some(ReporterOutput::Received {
                observer: DiagnosticReply::Completed {
                    bytes: received.bytes().len() as u64,
                    sha256: Sha256::digest(received.bytes()).into(),
                    revision: None,
                },
                received,
                admission,
                observation,
                request_context,
            });
            return Ok(());
        }
        if self.control_write.is_none()
            && let Some(input) = &self.control
        {
            let command = match input.command {
                ReporterControl::Copy(result) => {
                    DiagnosticCommand::CopyResult { session: input.session, result }
                }
                ReporterControl::Export(result) => {
                    DiagnosticCommand::ExportSelectionResult { session: input.session, result }
                }
                ReporterControl::RestartUnavailable => {
                    DiagnosticCommand::RestartUnavailable { session: input.session }
                }
                ReporterControl::Close(acknowledgement) => {
                    DiagnosticCommand::CloseAfterDurableAck {
                        session: input.session,
                        acknowledgement,
                    }
                }
            };
            self.control_write = parent
                .send(
                    input.until,
                    DiagnosticFrame {
                        key: input.key,
                        request: input.request,
                        context: input.context,
                        command,
                    },
                )
                .map_err(|_| ReporterSourceIssue::Parent)?;
        }
        if self.control_write.is_none()
            && self.control.is_none()
            && let Some(view) = self.views.iter_mut().flatten().next()
        {
            view.send(parent)?;
        }
        Ok(())
    }
}
fn observation(frame: DiagnosticFrame<'_>) -> Result<ReporterObservation, DiagnosticWireError> {
    let incident = frame.context.incident.ok_or(DiagnosticWireError::Shape)?;
    if frame.context.operation.is_some()
        || frame.context.deadline.is_some()
            != matches!(frame.command, DiagnosticCommand::CopyResult { .. })
        || frame.context.at.is_some()
    {
        return Err(DiagnosticWireError::Shape);
    }
    let binding = |session| ReportBinding::new(incident.key, incident.revision, session);
    Ok(match frame.command {
        DiagnosticCommand::Presented(session) => {
            ReporterObservation::Presented { binding: binding(session), replacement: false }
        }
        DiagnosticCommand::ViewReplaced(session) => {
            ReporterObservation::Presented { binding: binding(session), replacement: true }
        }
        DiagnosticCommand::Action { session, action } => {
            ReporterObservation::Action(ReportCommand::new(binding(session), action))
        }
        DiagnosticCommand::CopyResult { session, result } => {
            ReporterObservation::Copy { binding: binding(session), result }
        }
        DiagnosticCommand::SaveSelectionReady { session, selection } => {
            ReporterObservation::Selected {
                binding: binding(session),
                selection: selection.selection,
            }
        }
        DiagnosticCommand::SaveSelectionCancelled(session) => {
            ReporterObservation::SelectionCancelled(binding(session))
        }
        DiagnosticCommand::NativeRetired(session) => {
            ReporterObservation::NativeRetired(binding(session))
        }
        DiagnosticCommand::ReporterFailed { session, failure } => {
            ReporterObservation::Failed { binding: binding(session), failure }
        }
        _ => return Err(DiagnosticWireError::Role),
    })
}
