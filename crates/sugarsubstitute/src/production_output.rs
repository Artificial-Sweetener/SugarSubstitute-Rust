// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ingest actual coordinator output off the UI thread with one pull/frame/job in flight.
//! The composing host owns channel authentication, scopes and execution shutdown.
//! A wire reply grants matching data only. Delivery ACK advances only after the
//! actual finite parser result is observed; EOF never establishes process retirement.
mod errors;
mod recovery;
pub(crate) use recovery::BackendNotice;
use recovery::RequestKind;
mod snapshot;
mod state;
mod worker;
pub(crate) use errors::OutputError;
pub(crate) use snapshot::OutputSnapshot;
use std::num::NonZeroU64;
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_execution::{ExecutionClient, windows_lifecycle_pipe::ReceivedLifecycleFrame};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FrameCodec, LeaseReference, WireHeader, WireMessage, WirePayload,
};
/// Existing application's conservative transcript allocation bound; not a new cap.
pub(crate) const SOURCE_BYTES: u64 =
    sugarsubstitute_application::transcript::MAX_OWNER_ALLOCATION_BYTES as u64;
/// One existing32KiB bounded batch copy, separately admitted before allocating it.
pub(crate) const TRANSFER_BYTES: u64 =
    sugarsubstitute_contracts::transcript::MAX_BATCH_BYTES as u64;
/// Matching facts supplied after the composing channel/resource authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputBinding {
    /// Exact transcript resource including coordinator and acquisition generation.
    pub(crate) resource: ResourceKey,
    /// Current registered child run.
    pub(crate) run: RunKey,
    /// Current authenticated channel connection.
    pub(crate) connection: ConnectionId,
    /// Exact managed lease whose output is being read.
    pub(crate) lease: LeaseReference,
}
#[derive(Clone, Copy)]
struct Pending {
    header: WireHeader,
    original: Moment,
    admitted: bool,
    kind: RequestKind,
}
/// Successful finite parser result; the original guard survives semantic consumption.
pub(crate) struct AppliedOutput {
    /// Exact original request to pass to the channel's consume_reply.
    pub(crate) header: WireHeader,
    /// Original source frame, retained until the composing owner finishes consumption.
    pub(crate) frame: ReceivedLifecycleFrame,
    /// Actual accepted transcript mutation, or no mutation for OutputIdle.
    pub(crate) change: Option<sugarsubstitute_contracts::transcript::TranscriptChange>,
    /// Bounded styled view with original source custody for presentation.
    pub(crate) snapshot: Option<OutputSnapshot>,
    /// Mounted-backend notice parsed under this same original frame and request.
    pub(crate) notice: Option<BackendNotice>,
}
/// Own one transcript source, one real frame and one finite parser job.
/// Dropping while work is active keeps source/frame with the worker; the host must
/// observe the actual job and scope/worker settlement before claiming shutdown.
pub(crate) struct OutputOwner {
    source: state::Source,
    binding: OutputBinding,
    main_identity: Option<sugarsubstitute_contracts::startup::MainIdentity>,
    pending: Option<Pending>,
    acknowledged: Option<NonZeroU64>,
    job: Option<OwnedJob<worker::Parsed, OutputError>>,
    noticed: Option<TerminalKind>,
    terminal: Option<JobCompletion<worker::Parsed, OutputError>>,
    closed: bool,
}
impl OutputOwner {
    /// Admit a finite constructor and claim16MiB before any transcript/control allocation.
    /// The caller consumes its actual completion through the central routing owner.
    /// # Errors
    /// Preserves genuine admission/claim/control errors; no native operation occurs.
    pub(crate) fn submit_new(
        client: &ExecutionClient,
        scope: &Scope,
        job: JobId,
        binding: OutputBinding,
        original: Moment,
    ) -> Result<OwnedJob<Self, OutputError>, AdmissionError> {
        client.submit_blocking(scope, spec(job, original, SOURCE_BYTES), move |context| {
            if binding.lease.resource != binding.resource.id
                || binding.lease.generation != binding.resource.generation
                || binding.run.owner != binding.resource.owner
            {
                return Err(OutputError::Identity);
            }
            Ok(Self {
                source: state::Source::new(binding, &context)?,
                binding,
                main_identity: None,
                pending: None,
                acknowledged: None,
                job: None,
                noticed: None,
                terminal: None,
                closed: false,
            })
        })
    }
    /// Retain the channel's exact original pair before attempting its native send.
    /// # Errors
    /// Rejects duplicate/closed pulls or full run/connection/request mismatch.
    pub(crate) fn prepare_pull(
        &mut self,
        header: WireHeader,
        original: Moment,
    ) -> Result<WireMessage<'static>, OutputError> {
        self.prepare_request(header, original, RequestKind::Pull(self.main_identity))
    }
    /// Record only after the actual channel accepted the prepared request.
    /// # Errors
    /// Rejects duplicate/changed admission without releasing its original pair.
    pub(crate) fn admitted_request(&mut self, header: WireHeader) -> Result<(), OutputError> {
        let pending = self.pending.as_mut().ok_or(OutputError::Order)?;
        if pending.header != header || pending.admitted || self.closed {
            return Err(OutputError::Order);
        }
        pending.admitted = true;
        Ok(())
    }
    /// Move a genuine frame before admitting finite worker parsing; UI performs no text parsing.
    /// # Errors
    /// Rejection leaves the identical guard in the caller slot until adoption; after adoption
    /// this owner retains it through actual worker settlement, including admission failure.
    pub(crate) fn submit_frame(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        job: JobId,
        frame: &mut Option<ReceivedLifecycleFrame>,
    ) -> Result<(), OutputError> {
        if self.closed
            || self.job.is_some()
            || self.terminal.is_some()
            || !self.pending.is_some_and(|p| p.admitted)
        {
            return Err(OutputError::Order);
        }
        let Some(pending) = self.pending else {
            return Err(OutputError::Order);
        };
        let mut state = match self.source.state.lock() {
            Ok(s) => s,
            Err(_) => {
                return Err(OutputError::Poisoned);
            }
        };
        if state.frame.is_some() {
            return Err(OutputError::Order);
        }
        state.frame = Some(frame.take().ok_or(OutputError::Order)?);
        drop(state);
        let mut source = Some(self.source.clone());
        let binding = self.binding;
        let acknowledged = self.acknowledged;
        match client.submit_blocking_owned_input(
            scope,
            spec(job, pending.original, TRANSFER_BYTES),
            &mut source,
            move |source, context| worker::parse(&source, binding, pending, acknowledged, &context),
        ) {
            Ok(job) => {
                self.job = Some(job);
                self.noticed = None;
                Ok(())
            }
            Err(error) => Err(OutputError::Input(error)),
        }
    }
    /// Capture only actual matching completion notices; foreign jobs cannot consume this slot.
    pub(crate) fn capture(&mut self, batch: &CompletionBatch) {
        if let Some(job) = &self.job {
            for n in &batch.notifications {
                if (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id) {
                    self.noticed = Some(n.kind)
                }
            }
        }
    }
    /// Consume one actual successful worker result before permitting a delivery ACK.
    /// # Errors
    /// Failed/cancelled/panicked/mismatched completions retain original frame and terminal data.
    pub(crate) fn take_applied(&mut self) -> Result<Option<AppliedOutput>, OutputError> {
        let Some(noticed) = self.noticed else { return Ok(None) };
        let Some(job) = &mut self.job else { return Ok(None) };
        let Some(completion) = job.try_take_completion() else { return Ok(None) };
        let matching = completion.scope_id == job.scope_id()
            && completion.spec == job.spec()
            && completion.outcome.kind() == noticed;
        self.job = None;
        self.noticed = None;
        if !matching || self.closed || completion.outcome.kind() != TerminalKind::Success {
            let kind = completion.outcome.kind();
            self.terminal = Some(completion);
            self.closed = true;
            return Err(if matching { OutputError::Terminal(kind) } else { OutputError::Identity });
        }
        let Some(pending) = self.pending else {
            self.terminal = Some(completion);
            self.closed = true;
            return Err(OutputError::Order);
        };
        let frame = match self.source.state.lock() {
            Ok(mut state) => state.frame.take(),
            Err(_) => {
                self.terminal = Some(completion);
                self.closed = true;
                return Err(OutputError::Poisoned);
            }
        };
        let Some(frame) = frame else {
            self.terminal = Some(completion);
            self.closed = true;
            return Err(OutputError::Order);
        };
        let JobOutcome::Success(parsed) = completion.outcome else {
            self.closed = true;
            return Err(OutputError::Identity);
        };
        self.pending = None;
        if let Some(delivery) = parsed.delivery {
            self.acknowledged = Some(delivery)
        }
        Ok(Some(AppliedOutput {
            header: pending.header,
            frame,
            change: parsed.change,
            snapshot: parsed.snapshot,
            notice: parsed.notice,
        }))
    }
    /// Inspect the original pull during retirement; copied facts grant no native authority.
    pub(crate) fn retiring_pull(&self) -> Option<(WireHeader, Moment, bool)> {
        self.closed.then(|| self.pending.map(|p| (p.header, p.original, p.admitted))).flatten()
    }
    /// Classify only an actually observed terminal caused by intentional close. Successful
    /// closed work remains retained, and genuine failures/panics are never expected cancellation.
    pub(crate) fn expected_retirement_terminal(&self, cause: &OutputError) -> bool {
        if !self.closed || self.job.is_some() {
            return false;
        }
        let Some(terminal) = &self.terminal else { return false };
        matches!(
            (cause, &terminal.outcome),
            (OutputError::Terminal(TerminalKind::Success), JobOutcome::Success(_))
                | (OutputError::Terminal(TerminalKind::Cancelled), JobOutcome::Cancelled)
                | (
                    OutputError::Terminal(TerminalKind::Failure),
                    JobOutcome::Failure(OutputError::Cancelled)
                )
        )
    }
    /// Validate one original retiring reply without text parsing, jobs, publication or a
    /// delivery ACK. The composing channel has already authenticated its actual frame.
    pub(crate) fn validate_retiring_reply(&self, bytes: &[u8]) -> Result<WireHeader, OutputError> {
        use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, ReplyOutcome};
        if !self.closed || self.job.is_some() {
            return Err(OutputError::Order);
        }
        let pending = self.pending.filter(|p| p.admitted).ok_or(OutputError::Order)?;
        let message = FrameCodec::decode(bytes)?;
        if message.header != pending.header {
            return Err(OutputError::Identity);
        }
        match message.payload {
            DecodedPayload::Reply(ReplyOutcome::OutputIdle(lease))
                if lease == self.binding.lease && matches!(pending.kind, RequestKind::Pull(_)) => {}
            DecodedPayload::Reply(ReplyOutcome::OutputDelivery(delivery))
                if delivery.lease == self.binding.lease
                    && matches!(pending.kind, RequestKind::Pull(_))
                    && Some(delivery.delivery_id.get())
                        == self.acknowledged.map_or(Some(1), |id| id.get().checked_add(1)) => {}
            DecodedPayload::Reply(reply)
                if recovery::notice(self.binding, pending, message.header, reply)?.is_some() => {}
            _ => return Err(OutputError::Identity),
        }
        Ok(message.header)
    }
    /// Retire only exact pull metadata after the channel withdrew an unadmitted request or
    /// consumed its actual reply. Native frame disposal remains with the composing owner.
    pub(crate) fn finish_retiring_pull(&mut self, header: WireHeader) -> Result<(), OutputError> {
        if !self.closed || self.job.is_some() {
            return Err(OutputError::Order);
        }
        if self.pending.is_some_and(|pending| pending.header != header) {
            return Err(OutputError::Identity);
        }
        if self.source.state.lock().map_err(|_| OutputError::Poisoned)?.frame.is_some() {
            return Err(OutputError::Order);
        }
        self.pending = None;
        Ok(())
    }
    /// Close admission and request actual job cancellation without claiming it has stopped.
    pub(crate) fn close(&mut self) {
        self.closed = true;
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    /// Observe whether an actual parser job still needs its terminal completion.
    pub(crate) const fn worker_pending(&self) -> bool {
        self.job.is_some()
    }
    /// Borrow the exact retained failure/panic outcome for the composing failure owner.
    pub(crate) fn failure(&self) -> Option<&OutputError> {
        match self.terminal.as_ref().map(|c| &c.outcome) {
            Some(JobOutcome::Failure(e)) => Some(e),
            _ => None,
        }
    }
    /// Borrow the actual bounded supervised panic without replacing its cause.
    pub(crate) fn panic_cause(&self) -> Option<&PanicCause> {
        match self.terminal.as_ref().map(|c| &c.outcome) {
            Some(JobOutcome::Panicked(e)) => Some(e),
            _ => None,
        }
    }
    /// Transfer an unconsumed original frame only after the actual parser job ended.
    /// # Errors
    /// A live worker or poisoned retained state blocks transfer, preserving custody.
    pub(crate) fn take_unparsed_frame(
        &mut self,
    ) -> Result<Option<ReceivedLifecycleFrame>, OutputError> {
        if self.job.is_some() {
            return Err(OutputError::Order);
        }
        Ok(self.source.state.lock().map_err(|_| OutputError::Poisoned)?.frame.take())
    }
}
fn spec(id: JobId, deadline: Moment, bytes: u64) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(deadline),
        reservation_bytes: bytes,
        replacement: None,
    }
}
#[cfg(test)]
mod tests;
