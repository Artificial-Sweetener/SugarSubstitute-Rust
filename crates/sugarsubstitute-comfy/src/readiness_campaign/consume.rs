// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance only after exact central metadata and the actual typed owner's result.
use super::*;
use crate::readiness_transport::ProbeRetryDisposition;
use std::time::Duration;
use sugarsubstitute_contracts::execution::{CompletionBatch, JobOutcome};

impl ReadinessCampaign {
    /// Consume matching actual attempt/cleanup completion and original-time progression.
    ///
    /// Batch metadata alone creates no proof: the retained real job transfers its
    /// terminal result separately. Stale/foreign notifications are ignored. Empty
    /// batches advance original backoff/expiry only, never actual job settlement.
    /// # Errors
    /// Clock regression closes admission and requests actual pending cancellation
    /// while retaining jobs, native outputs and their original deadlines. Cleanup failures return
    /// their original cause, close retry admission and retain the probe's custody.
    pub fn consume(
        &mut self,
        batch: &CompletionBatch,
        now: Moment,
    ) -> Result<CampaignStep, CampaignError> {
        self.observe_now(now)?;
        self.last_now = now;
        if !self.ready && now >= self.budget.until() && !self.terminal {
            self.terminal = true;
            self.retry = false;
            self.next_input_at = None;
            self.failure = Some(CampaignError::Budget(ProbeBudgetError::Expired));
            if let Some(owner) = &self.owner {
                owner.request_cancel();
            }
        }
        if let Some(attempt) = &self.attempt
            && batch
                .notifications
                .iter()
                .any(|item| item.job_id == attempt.job && item.scope_id == attempt.scope)
        {
            let owner = self
                .owner
                .as_mut()
                .ok_or(CampaignError::Rejected(CampaignRejection::WrongPhase))?;
            match owner.take_readiness() {
                Ok(None) if owner.continuation_ready() => {
                    self.attempt = None;
                    return Ok(if self.terminal {
                        CampaignStep::Failed
                    } else {
                        CampaignStep::NeedsContinuation
                    });
                }
                Ok(None) => return Ok(CampaignStep::Waiting),
                Ok(Some(proof)) if !self.terminal => {
                    self.attempt = None;
                    self.ready = true;
                    self.readiness = Some(proof);
                    return Ok(CampaignStep::Ready);
                }
                Ok(Some(_)) => self.attempt = None,
                Err(cause) => {
                    self.attempt = None;
                    self.retry = !self.terminal
                        && cause.retry_disposition()
                            == ProbeRetryDisposition::TransientConnectionUnavailable;
                    if !self.retry {
                        self.terminal = true;
                    }
                    let prior_transient = matches!(&self.failure, Some(CampaignError::Transport(previous)) if previous.retry_disposition() == ProbeRetryDisposition::TransientConnectionUnavailable);
                    if self.failure.is_none() || prior_transient {
                        self.failure = Some(CampaignError::Transport(cause));
                    } else {
                        return Err(CampaignError::Transport(cause));
                    }
                }
            }
            return Ok(if self.terminal { CampaignStep::Failed } else { CampaignStep::Progressed });
        }
        if let Some(job) = &mut self.cleanup
            && batch
                .notifications
                .iter()
                .any(|item| item.job_id == job.spec().id && item.scope_id == job.scope_id())
            && let Some(completion) = job.try_take_completion()
        {
            self.cleanup = None;
            match completion.outcome {
                JobOutcome::Success(()) => {
                    self.owner = None;
                    self.cleanup_until = None;
                    self.readiness = None;
                    if self.retry && !self.terminal {
                        let delays_ms = [50, 100, 200, 400, 800, 1000];
                        let delay = delays_ms[self.retry_index.min(delays_ms.len() - 1)];
                        self.retry_index = (self.retry_index + 1).min(delays_ms.len() - 1);
                        let remaining = self
                            .budget
                            .until()
                            .checked_duration_since(now)
                            .map_err(CampaignError::Time)?;
                        self.next_input_at = Some(
                            now.checked_add(remaining.min(Duration::from_millis(delay)))
                                .map_err(CampaignError::Time)?,
                        );
                        self.retry = false;
                        return Ok(CampaignStep::Progressed);
                    }
                    self.terminal = true;
                    self.retry = false;
                    self.next_input_at = None;
                    return Ok(CampaignStep::Failed);
                }
                JobOutcome::Failure(cause) => {
                    self.terminal = true;
                    self.retry = false;
                    return Err(CampaignError::Transport(cause));
                }
                JobOutcome::Panicked(cause) => {
                    self.terminal = true;
                    self.retry = false;
                    return Err(CampaignError::CleanupPanicked(cause));
                }
                outcome => {
                    self.terminal = true;
                    self.retry = false;
                    return Err(CampaignError::CleanupOutcome(outcome.kind()));
                }
            }
        }
        if self.attempt.is_some() || self.cleanup.is_some() {
            return Ok(CampaignStep::Waiting);
        }
        if self.terminal {
            Ok(CampaignStep::Failed)
        } else if self.ready {
            Ok(CampaignStep::Ready)
        } else if self.needs_continuation(now)? {
            Ok(CampaignStep::NeedsContinuation)
        } else if self.needs_input(now)? {
            Ok(CampaignStep::NeedsInput)
        } else {
            Ok(CampaignStep::Waiting)
        }
    }
}
