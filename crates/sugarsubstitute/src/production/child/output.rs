// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume genuine managed output before acknowledging its delivery on the original channel.
use super::*;
use crate::production_output::{OutputBinding, OutputError};
use std::time::Duration;
use sugarsubstitute_supervision::lifecycle_protocol::FrameCodec;
type Constructor = OwnedJob<OutputOwner, OutputError>;
type Terminal = JobCompletion<OutputOwner, OutputError>;
pub(super) struct ChildOutput {
    job: Option<Constructor>,
    notice: Option<TerminalKind>,
    terminal: Option<Terminal>,
    pending: Option<(WireHeader, Moment)>,
    closed: bool,
    applied: Option<crate::production_output::AppliedOutput>,
    retiring_frame: Option<ReceivedLifecycleFrame>,
    retirement_error_reported: bool,
}
impl ChildOutput {
    pub(super) fn new() -> Self {
        Self {
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
        if self.closed || self.job.is_some() || self.terminal.is_some() {
            return Err("Managed output constructor duplicated".into());
        }
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
    fn owner_mut(&mut self) -> Option<&mut OutputOwner> {
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
            let Some(completion) = job.try_take_completion() else { return Ok(()) };
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
            if let Err(cause) = owner.take_applied() {
                if owner.expected_retirement_terminal(&cause) {
                    return Ok(());
                }
                self.output.retirement_error_reported = true;
                return Err(Box::new(cause));
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
        if !self.main.profile().is_some_and(profile_transaction::ProfileTransaction::authorized) {
            return Ok(());
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
            let header = self.bootstrap.channel_mut().prepare_request(original)?;
            let Some(owner) = self.output.owner_mut() else { return Ok(()) };
            owner.prepare_pull(header, original)?;
            self.output.pending = Some((header, original));
        }
        let pending = self.output.owner_mut().and_then(|owner| owner.pending_pull());
        if let Some((message, original)) = pending
            && self.bootstrap.channel_mut().try_send(&message, original)?
        {
            self.output
                .owner_mut()
                .ok_or("Output source disappeared")?
                .admitted_pull(message.header)?;
        }
        Ok(())
    }
}

impl ProductionChild {
    /// Drain only the original closed pull and actual ended parser; issue no jobs or new pulls.
    fn retire_output(&mut self) -> Result<()> {
        if self.output.job.is_some()
            || self.output.owner_mut().is_some_and(|owner| owner.worker_pending())
        {
            return Ok(());
        }
        let Some((header, _original)) = self.output.pending else {
            return Ok(());
        };
        let pending =
            self.output.owner_mut().ok_or("Original output owner missing")?.retiring_pull();
        if let Some((actual, _, false)) = pending {
            if actual != header {
                return Err("Retiring output header differs".into());
            }
            self.bootstrap.channel_mut().withdraw_prepared(header)?;
            self.output
                .owner_mut()
                .ok_or("Original output owner missing")?
                .finish_retiring_pull(header)?;
            self.output.pending = None;
            return Ok(());
        }
        if self.output.applied.is_none() && self.output.retiring_frame.is_none() {
            self.output.retiring_frame = self
                .output
                .owner_mut()
                .ok_or("Original output owner missing")?
                .take_unparsed_frame()?;
            if self.output.retiring_frame.is_none()
                && self.failed_frame.as_ref().is_some_and(|frame| {
                    FrameCodec::decode(frame.as_bytes())
                        .is_ok_and(|message| message.header == header)
                })
            {
                self.output.retiring_frame = self.failed_frame.take();
            }
            if self.output.retiring_frame.is_none()
                && self.bootstrap.channel_mut().received_header()? == Some(header)
            {
                self.output.retiring_frame = self.bootstrap.channel_mut().take_frame();
            }
        }
        if let Some(applied) = &self.output.applied {
            if applied.header != header {
                return Err("Applied output retirement header differs".into());
            }
        } else if let Some(frame) = &self.output.retiring_frame {
            let bytes = frame.as_bytes();
            let owner = match self.output.terminal.as_ref().map(|terminal| &terminal.outcome) {
                Some(JobOutcome::Success(owner)) => owner,
                _ => return Err("Original output source missing".into()),
            };
            if owner.validate_retiring_reply(bytes)? != header {
                return Err("Retiring output reply differs".into());
            }
        } else {
            return Ok(());
        }
        self.bootstrap.channel_mut().consume_reply(header)?;
        self.output
            .owner_mut()
            .ok_or("Original output owner missing")?
            .finish_retiring_pull(header)?;
        self.output.pending = None;
        self.output.applied = None;
        self.output.retiring_frame = None;
        tracing::info!(operation="closed_output_pull_retired",request=?header.request,run=?self.run,
            "Original output reply consumed after actual parser retirement");
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
