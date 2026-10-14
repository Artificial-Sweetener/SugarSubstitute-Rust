// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume genuine managed output before acknowledging its delivery on the original channel.
use super::*;
use crate::production_output::{OutputBinding, OutputError};
use std::time::Duration;
use sugarsubstitute_supervision::lifecycle_protocol::FrameCodec;
#[path = "output/acquisition.rs"]
mod acquisition;
#[path = "output/retirement.rs"]
mod retirement;
type Constructor = OwnedJob<OutputOwner, OutputError>;
type Terminal = JobCompletion<OutputOwner, OutputError>;
pub(super) struct ChildOutput {
    pub(super) binding: Option<OutputBinding>,
    acquisition_until: Option<Moment>,
    job: Option<Constructor>,
    notice: Option<TerminalKind>,
    terminal: Option<Terminal>,
    pub(super) pending: Option<(WireHeader, Moment)>,
    closed: bool,
    applied: Option<crate::production_output::AppliedOutput>,
    retiring_frame: Option<ReceivedLifecycleFrame>,
    retirement_error_reported: bool,
}
impl ChildOutput {
    pub(super) fn new() -> Self {
        Self {
            binding: None,
            acquisition_until: None,
            job: None,
            notice: None,
            terminal: None,
            pending: None,
            closed: false,
            applied: None,
            retiring_frame: None,
            retirement_error_reported: false,
        }
    }
    pub(super) fn begin(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        id: JobId,
        binding: OutputBinding,
        original: Moment,
    ) -> Result<()> {
        if self.closed || self.binding.is_some() || self.job.is_some() || self.terminal.is_some() {
            return Err("Managed output constructor duplicated".into());
        }
        self.binding = Some(binding);
        self.acquisition_until = Some(original);
        self.job = Some(OutputOwner::submit_new(client, scope, id, binding, original)?);
        Ok(())
    }
    pub(super) fn capture(&mut self, batch: &CompletionBatch) {
        if let Some(job) = &self.job {
            for n in &batch.notifications {
                if (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id) {
                    self.notice = Some(n.kind)
                }
            }
        }
        if let Some(owner) = self.owner_mut() {
            owner.capture(batch)
        }
    }
    pub(super) fn owner_mut(&mut self) -> Option<&mut OutputOwner> {
        match &mut self.terminal.as_mut()?.outcome {
            JobOutcome::Success(owner) => Some(owner),
            _ => None,
        }
    }
    /// Observe only this attempt's closed admission, not native or worker settlement.
    pub(super) fn closing(&self) -> bool {
        self.closed
    }
    /// Keep the original output reply with this owner through control draining.
    pub(super) fn owns_reply(&self, header: WireHeader) -> bool {
        self.pending.is_some_and(|pending| pending.0 == header)
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.pending.map(|p| p.1).or_else(|| self.job.as_ref().and_then(|j| j.spec().deadline))
    }
    pub(super) fn close(&mut self) {
        self.closed = true;
        if let Some(job) = &self.job {
            job.cancel()
        }
        if let Some(owner) = self.owner_mut() {
            owner.close()
        }
    }
    pub(super) fn release_frame(&mut self) -> Result<()> {
        if self.pending() {
            return Err("Original output pull or worker has not retired".into());
        }
        if let Some(owner) = self.owner_mut() {
            drop(owner.take_unparsed_frame()?);
        }
        self.applied = None;
        Ok(())
    }
    /// Actual finite construction/parser/frame custody; correlation alone is not a worker.
    pub(super) fn physical_pending(&self) -> bool {
        self.job.is_some()
            || self.applied.is_some()
            || self.retiring_frame.is_some()
            || match self.terminal.as_ref().map(|t| &t.outcome) {
                Some(JobOutcome::Success(owner)) => owner.worker_pending(),
                _ => false,
            }
    }
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
            || self.pending.is_some()
            || self.applied.is_some()
            || self.retiring_frame.is_some()
            || match self.terminal.as_ref().map(|t| &t.outcome) {
                Some(JobOutcome::Success(owner)) => owner.worker_pending(),
                _ => false,
            }
    }
}
impl ProductionChild {
    pub(super) fn consume_output(&mut self) -> Result<()> {
        if self.output.terminal.is_none() && self.output.notice.is_some() {
            let job = self.output.job.as_mut().ok_or("Output constructor absent")?;
            let Some(completion) = job.try_take_completion() else {
                return Ok(());
            };
            let matches = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && Some(completion.outcome.kind()) == self.output.notice;
            self.output.terminal = Some(completion);
            self.output.job = None;
            self.output.notice = None;
            if !matches {
                return Err("Output constructor completion metadata differs".into());
            }
        }
        if self.output.closed {
            if self.output.retirement_error_reported {
                return Ok(());
            }
            let Some(owner) = self.output.owner_mut() else {
                if let Some(terminal) = &self.output.terminal {
                    let expected = matches!(
                        &terminal.outcome,
                        JobOutcome::Cancelled | JobOutcome::Failure(OutputError::Cancelled)
                    );
                    if !expected {
                        let kind = terminal.outcome.kind();
                        self.output.retirement_error_reported = true;
                        return Err(Box::new(OutputError::Terminal(kind)));
                    }
                }
                return Ok(());
            };
            owner.close();
            match owner.take_applied() {
                Ok(Some(applied)) => self.output.applied = Some(applied),
                Ok(None) => {}
                Err(cause) if owner.expected_retirement_terminal(&cause) => return Ok(()),
                Err(cause) => {
                    self.output.retirement_error_reported = true;
                    return Err(Box::new(cause));
                }
            }
            return Ok(());
        }
        if self.output.applied.is_none() {
            let Some(owner) = self.output.owner_mut() else {
                return if self.output.terminal.is_some() && !self.shutdown.closing() {
                    Err("Output constructor failed with retained actual terminal".into())
                } else {
                    Ok(())
                };
            };
            if self.shutdown.closing() {
                owner.close();
            }
            match owner.take_applied() {
                Ok(Some(applied)) => self.output.applied = Some(applied),
                Ok(None) => return Ok(()),
                Err(cause) if self.shutdown.closing() => {
                    tracing::debug!(
                        operation = "output_ingestion",
                        failure_retained = owner.failure().is_some(),
                        panic_retained = owner.panic_cause().is_some(),
                        "Observed retained output terminal"
                    );
                    if self.failure.is_none() {
                        self.failure = Some(Box::new(cause));
                    }
                    return Ok(());
                }
                Err(cause) => {
                    tracing::debug!(
                        operation = "output_ingestion",
                        failure_retained = owner.failure().is_some(),
                        panic_retained = owner.panic_cause().is_some(),
                        "Observed retained output terminal"
                    );
                    return Err(Box::new(cause));
                }
            }
        }
        if self.shutdown.closing() {
            return Ok(());
        }
        let header = self.output.applied.as_ref().ok_or("Applied output disappeared")?.header;
        self.bootstrap.channel_mut().consume_reply(header)?;
        let applied = self
            .output
            .applied
            .take()
            .ok_or("Applied output disappeared after metadata consumption")?;
        self.output.pending = None;
        tracing::debug!(operation="output_ingestion",change=?applied.change,source_bytes=applied.snapshot.as_ref().map(OutputSnapshot::source_bytes),"Consumed actual output revision");
        self.snapshot = applied.snapshot.or(self.snapshot.take());
        drop(applied.frame);
        Ok(())
    }
    pub(super) fn advance_output(&mut self) -> Result<()> {
        if self.output.closed {
            return self.retire_output();
        }
        if let Some((header, _)) = self.output.pending
            && self.bootstrap.channel_mut().received_header()? == Some(header)
        {
            let frame = self
                .bootstrap
                .channel_mut()
                .take_frame()
                .ok_or("Actual output frame disappeared")?;
            self.failed_frame = Some(frame);
            let id = self.next_job()?;
            let client = self.client()?.clone();
            let scope = self.bootstrap.scope().ok_or("Actual output scope absent")?;
            let owner = self.output.owner_mut().ok_or("Output source absent")?;
            owner.submit_frame(&client, scope, id, &mut self.failed_frame)?;
        }
        if self.output.owner_mut().is_none() {
            return Ok(());
        }
        if self.output.pending.is_none() {
            let client = self.client()?.clone();
            let original = client.now()?.checked_add(Duration::from_secs(30))?;
            let original =
                self.output.acquisition_until.map_or(original, |cutoff| original.min(cutoff));
            let header = self.bootstrap.channel_mut().prepare_request(original)?;
            let Some(owner) = self.output.owner_mut() else {
                return Ok(());
            };
            owner.prepare_pull(header, original)?;
            self.bootstrap.channel_mut().output_pull(header)?;
            self.output.pending = Some((header, original));
        }
        let pending = self.output.owner_mut().and_then(|owner| owner.pending_pull());
        if let Some((message, original)) = pending
            && self.bootstrap.channel_mut().try_send(&message, original).or_else(
                |cause| -> Result<bool> {
                    self.recover_acquisition_expiry(cause)?;
                    Ok(false)
                },
            )?
        {
            self.output
                .owner_mut()
                .ok_or("Output source disappeared")?
                .admitted_pull(message.header)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_keeps_pending_pull_with_output_owner_and_blocks_early_frame_release() -> Result<()> {
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        let header = WireHeader {
            run,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(2)?),
            deadline_qpc: 100,
        };
        let mut output = ChildOutput::new();
        output.pending = Some((header, Moment::from_nanos(100)));
        output.close();
        assert!(output.closing() && output.pending());
        assert!(output.owns_reply(header));
        assert!(!output.owns_reply(WireHeader { request: Some(RequestId::new(3)?), ..header }));
        assert!(output.release_frame().is_err());
        assert_eq!(output.deadline(), Some(Moment::from_nanos(100)));
        assert_eq!(output.pending, Some((header, Moment::from_nanos(100))));
        Ok(())
    }
}
