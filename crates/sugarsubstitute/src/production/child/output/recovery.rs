// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the old source and received pair until a genuinely funded successor source is adopted.
use super::*;
use crate::production_output::BackendNotice;
use sugarsubstitute_desktop::MainBackendState;
#[derive(Debug)]
enum RetryDisposition {
    Failed(LifecycleFailureCode),
    Rejected(LifecycleRejection),
}
impl std::fmt::Display for RetryDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(code) => write!(f, "Authenticated backend retry failed: {code:?}"),
            Self::Rejected(code) => write!(f, "Authenticated backend retry rejected: {code:?}"),
        }
    }
}
impl Error for RetryDisposition {}

pub(super) struct Replacement {
    identity: MainIdentity,
    previous: LeaseReference,
    binding: OutputBinding,
    job: Option<Constructor>,
    notice: Option<TerminalKind>,
    terminal: Option<Terminal>,
    cause: Option<Box<dyn Error>>,
}
impl Replacement {
    pub(super) fn capture(&mut self, batch: &CompletionBatch) {
        if let Some(job) = &self.job {
            for notice in &batch.notifications {
                if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                    self.notice = Some(notice.kind);
                }
            }
        }
    }
    pub(super) fn close(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
        if let Some(JobCompletion { outcome: JobOutcome::Success(owner), .. }) = &mut self.terminal
        {
            owner.close();
        }
    }
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }
    pub(super) fn stopped(&self) -> bool {
        self.cause.is_some()
    }
}
impl ChildOutput {
    pub(super) fn retry_pending(&mut self) -> bool {
        self.owner_mut().is_some_and(|owner| owner.pending_retry())
    }
}
impl ProductionChild {
    /// Mounted state is consumed once under the original real parsed frame/request.
    pub(super) fn consume_backend_notice(&mut self) -> Result<bool> {
        let Some(notice) = self.output.applied.as_ref().and_then(|applied| applied.notice) else {
            return Ok(false);
        };
        self.ensure_running_backend()?;
        if self.running_backend.is_none() {
            return Err("Backend notice precedes actual mounted Main".into());
        }
        match notice {
            BackendNotice::Unavailable { failed, status, affected_clients, retry_ready } => {
                let running = self.running_backend.as_mut().ok_or("Mounted backend absent")?;
                let changed = running.unavailable(failed, retry_ready)?;
                let failed = running.key(failed);
                if let Some((state, previous)) = changed {
                    let command = match previous {
                        Some(previous) => DesktopStartupCommand::ReplaceBackendOutage {
                            identity: self.identity,
                            previous,
                            failed,
                            state,
                        },
                        None => DesktopStartupCommand::PresentBackendOutage {
                            identity: self.identity,
                            failed,
                            state,
                        },
                    };
                    self.push_command(command)?;
                    tracing::warn!(operation="mounted_backend_unavailable",identity=?self.identity,
                    failed=?failed,status,affected_clients,retry_ready,
                    "Authenticated mounted backend unavailable");
                }
            }
            BackendNotice::Replaced { identity, previous, current } => {
                if self.output.replacement.is_none() {
                    let old = self.output.binding.ok_or("Original output binding absent")?;
                    let running = self.running_backend.as_mut().ok_or("Mounted backend absent")?;
                    let failed = running.replacing(identity, previous, current)?;
                    self.push_command(DesktopStartupCommand::PresentBackendOutage {
                        identity,
                        failed,
                        state: MainBackendState::Retrying,
                    })?;
                    let binding = OutputBinding {
                        resource: ResourceKey {
                            owner: identity.run.owner,
                            id: current.resource,
                            generation: current.generation,
                        },
                        lease: current,
                        ..old
                    };
                    let original =
                        self.output.pending.ok_or("Actual replacement request absent")?.1;
                    // Admission funds both old source/snapshots and this new source before binding swap.
                    // An actual rejection is retained once; no repeated pump admission or renewed clock.
                    let id = self.next_job()?;
                    let admitted = OutputOwner::submit_new(
                        self.client()?,
                        self.bootstrap.scope().ok_or("Original child scope absent")?,
                        id,
                        binding,
                        original,
                    );
                    let (job, cause) = match admitted {
                        Ok(job) => (Some(job), None),
                        Err(cause) => (None, Some(Box::new(cause) as Box<dyn Error>)),
                    };
                    self.output.replacement = Some(Replacement {
                        identity,
                        previous,
                        binding,
                        job,
                        notice: None,
                        terminal: None,
                        cause,
                    });
                }
                return Ok(true);
            }
            BackendNotice::RetryFailed(code) => {
                if self.output.retry_cause.is_none() {
                    self.output.retry_cause = Some(Box::new(RetryDisposition::Failed(code)));
                }
                tracing::warn!(operation="mounted_backend_retry_failed",failure=?code,
                    "Original retry received its actual failure disposition");
                self.present_retry_failed()?;
            }
            BackendNotice::RetryRejected(code) => {
                if self.output.retry_cause.is_none() {
                    self.output.retry_cause = Some(Box::new(RetryDisposition::Rejected(code)));
                }
                tracing::warn!(operation="mounted_backend_retry_rejected",rejection=?code,
                    "Original retry received its actual rejection disposition");
                self.present_retry_failed()?;
            }
        }
        let header = self.output.applied.as_ref().ok_or("Original notice frame absent")?.header;
        self.bootstrap.channel_mut().consume_reply(header)?;
        self.output.pending = None;
        // Return the actual passive loan only after semantic and correlation consumption.
        self.output.applied = None;
        Ok(true)
    }
    fn present_retry_failed(&mut self) -> Result<()> {
        let failed =
            self.running_backend.as_mut().ok_or("Mounted backend absent")?.retry_failed()?;
        self.push_command(DesktopStartupCommand::PresentBackendOutage {
            identity: self.identity,
            failed,
            state: MainBackendState::Retiring,
        })
    }
    /// Observe the exact genuine constructor, retaining its original failure/terminal on rejection.
    pub(super) fn advance_output_replacement(&mut self) -> Result<()> {
        let Some(replacement) = &mut self.output.replacement else {
            return Ok(());
        };
        if replacement.terminal.is_none() && replacement.notice.is_some() {
            let job = replacement.job.as_mut().ok_or("Replacement constructor absent")?;
            if let Some(completion) = job.try_take_completion() {
                let exact = completion.scope_id == job.scope_id()
                    && completion.spec == job.spec()
                    && Some(completion.outcome.kind()) == replacement.notice;
                let success = matches!(&completion.outcome, JobOutcome::Success(_));
                let kind = completion.outcome.kind();
                replacement.terminal = Some(completion);
                replacement.job = None;
                if !exact || !success {
                    replacement.cause = Some(Box::new(if exact {
                        OutputError::Terminal(kind)
                    } else {
                        OutputError::Identity
                    }));
                }
            }
        }
        if self.output.closed || self.shutdown.closing() {
            replacement.close();
            return Ok(());
        }
        if replacement.cause.is_some() || replacement.terminal.is_none() {
            return Ok(());
        }
        let (identity, previous, binding) =
            (replacement.identity, replacement.previous, replacement.binding);
        self.running_backend.as_ref().ok_or("Mounted backend absent")?.validate_replacement(
            identity,
            previous,
            binding.lease,
        )?;
        if self.identity != identity
            || self.main.profile().is_none_or(|profile| !profile.authorized())
        {
            return Err("Current mounted Main retired before actual replacement admission".into());
        }
        let replacement = self.output.replacement.as_mut().ok_or("Replacement disappeared")?;
        let completion = replacement.terminal.as_mut().ok_or("Funded source disappeared")?;
        if let JobOutcome::Success(owner) = &mut completion.outcome {
            owner.running_main(identity)?;
        }
        let header = self.output.applied.as_ref().ok_or("Actual replacement frame absent")?.header;
        self.bootstrap.channel_mut().consume_reply(header)?;
        let replacement = self.output.replacement.as_mut().ok_or("Replacement disappeared")?;
        let completion = replacement.terminal.take().ok_or("Funded source disappeared")?;
        self.running_backend.as_mut().ok_or("Mounted backend absent")?.replaced(
            identity,
            previous,
            binding.lease,
        )?;
        self.lease = Some(binding.lease);
        self.output.pending = None;
        self.output.applied = None;
        // The old parser already ended and its loan/request retired; old snapshots keep old credit.
        self.output.terminal = Some(completion);
        self.output.binding = Some(binding);
        self.output.replacement = None;
        self.output.retry_expired = false;
        Ok(())
    }
    /// Stop an expired finite intent from scheduling past-deadline wakeups; retain its real request.
    pub(super) fn expire_backend_retry(&mut self) -> Result<()> {
        if self.running_backend.as_ref().and_then(|running| running.retry_deadline()).is_none() {
            return Ok(());
        }
        let now = self.client()?.now()?;
        let failed = self.running_backend.as_mut().and_then(|running| running.expire_retry(now));
        if let Some(failed) = failed {
            self.output.retry_expired = true;
            if self.output.retry_cause.is_none() {
                self.output.retry_cause = Some(Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Original managed retry deadline expired",
                )));
            }
            self.push_command(DesktopStartupCommand::PresentBackendOutage {
                identity: self.identity,
                failed,
                state: MainBackendState::Retiring,
            })?;
        }
        Ok(())
    }
    /// Queue user retry only after the existing old pull and actual parser/frame are consumed.
    pub(super) fn prepare_backend_retry(&mut self) -> Result<bool> {
        let retry = self.running_backend.as_ref().and_then(|running| running.retry_request());
        let Some((identity, failed, until)) = retry else {
            return Ok(false);
        };
        let header = self.bootstrap.channel_mut().prepare_request(until)?;
        self.output
            .owner_mut()
            .ok_or("Original retry output source absent")?
            .prepare_retry(header, until, identity, failed)?;
        self.bootstrap.channel_mut().backend_retry(header)?;
        self.output.pending = Some((header, until));
        Ok(true)
    }
    pub(super) fn recover_unissued_backend_retry(&mut self, cause: Box<dyn Error>) -> Result<bool> {
        if !self.output.retry_pending() {
            return Err(cause);
        }
        let (header, _) = self.output.pending.ok_or("Original retry correlation absent")?;
        let exact = cause.downcast_ref::<requests::RequestExpiry>().is_some_and(|error| {
            error.header == header && error.purpose == requests::Purpose::BackendRetry
        }) || cause.downcast_ref::<channel::expiry::NativeWriteExpiry>().is_some_and(
            |error| error.header == header && error.purpose == requests::Purpose::BackendRetry,
        );
        if !exact || !self.bootstrap.channel_mut().request_prepared(header)? {
            return Err(cause);
        }
        self.bootstrap.channel_mut().withdraw_prepared(header)?;
        self.output.owner_mut().ok_or("Original retry source absent")?.withdraw_retry(header)?;
        self.output.pending = None;
        if self.output.retry_cause.is_none() {
            self.output.retry_cause = Some(cause);
        }
        self.present_retry_failed()?;
        Ok(false)
    }
}
