// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquire selected encoded originals without coupling canvas state to a host's transport.

use super::*;
use std::io;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};
use sugarsubstitute_image_io::ImageSource;
use sugarsubstitute_supervision::cache::RuntimeCache;

/// An inert, one-use host adapter retained until its native acquisition actually retires.
/// Implementations never schedule work; the canvas runs these operations on admitted workers.
pub(crate) trait SourceAcquirer: Send + Sync {
    /// Peak transient bytes needed while acquiring the original.
    fn reservation_bytes(&self) -> u64;

    /// Acquire one retained original on an ordinary worker with the supplied finite context.
    /// Observe cancellation before returning; success requires native transfer retirement.
    fn acquire(
        &self,
        cache: RuntimeCache,
        resident: &MemoryLease,
        context: &JobContext,
    ) -> io::Result<ImageSource>;

    /// Stop an unstarted acquisition without blocking; running jobs are separately cancelled.
    fn cancel(&self);

    /// Retire remaining native custody on a fresh admitted worker from the original executor.
    fn retire(&self, context: &JobContext) -> io::Result<()>;

    /// Observe checked retirement without blocking the native event loop.
    fn settled(&self) -> bool;
}

/// Keep a failed or cancelled acquisition until its actual native handles have retired.
pub(super) struct Retirement {
    owner: Arc<dyn SourceAcquirer>,
    job: Option<OwnedJob<(), WorkError>>,
    cutoff: Option<Moment>,
    failed: bool,
    retry_on_close: bool,
}

impl Session {
    /// Retain the original adapter before attempting to derive or admit a cleanup deadline.
    pub(super) fn retain_acquirer(
        &mut self,
        owner: Arc<dyn SourceAcquirer>,
    ) -> Result<(), Box<dyn Error>> {
        if owner.settled() {
            return Ok(());
        }
        if self.acquisition_retirement.is_some() {
            return Err("Canvas acquisition retirement is already retained".into());
        }
        let cutoff = self.retirement_cutoff();
        self.acquisition_retirement = Some(Retirement {
            owner,
            job: None,
            cutoff: cutoff.as_ref().ok().copied(),
            failed: false,
            retry_on_close: false,
        });
        cutoff.map(|_| ())
    }

    /// Observe the real cleanup completion and admit at most one owned retirement job.
    pub(super) fn advance_acquirer_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        let completed = self
            .acquisition_retirement
            .as_mut()
            .and_then(|retirement| retirement.job.as_mut())
            .and_then(OwnedJob::try_take_completion);
        if let Some(completion) = completed {
            let retirement = self.acquisition_retirement.as_mut().ok_or("Acquisition vanished")?;
            let job = retirement.job.take().ok_or("Acquisition cleanup job vanished")?;
            Self::observe_terminal(&job, &completion.outcome);
            if retirement.owner.settled() {
                let failure = match completion.outcome {
                    JobOutcome::Success(()) => None,
                    JobOutcome::Cancelled if self.closing => None,
                    other => Self::terminal(other).err(),
                };
                self.acquisition_retirement = None;
                if let Some(cause) = failure {
                    self.fail(cause);
                }
                return Ok(());
            }
            let retry = self.closing && retirement.retry_on_close;
            retirement.retry_on_close = false;
            retirement.failed = !retry;
            if !retry {
                let cause = Self::terminal(completion.outcome)
                    .err()
                    .unwrap_or_else(|| "Acquisition cleanup retained native custody".into());
                self.fail(cause);
            }
        }
        let Some(retirement) = &self.acquisition_retirement else { return Ok(()) };
        if retirement.job.is_some() || retirement.failed {
            return Ok(());
        }
        let owner = retirement.owner.clone();
        let cutoff = retirement.cutoff.ok_or("Acquisition cleanup deadline is unavailable")?;
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut spec = self.spec(
            Operation::NativeLifecycle,
            JobClass::InfrastructureLifecycle,
            512 * 1024,
            Duration::from_secs(5),
            None,
        )?;
        spec.deadline = Some(cutoff);
        match runtime.client.submit_blocking(&runtime.scope, spec, move |context| {
            owner.retire(&context).map_err(WorkError::from)
        }) {
            Ok(job) => {
                self.acquisition_retirement.as_mut().ok_or("Acquisition vanished")?.job = Some(job);
            }
            Err(cause) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Tighten cleanup to the original shutdown budget and allow one fresh cleanup attempt.
    pub(super) fn close_acquirer_retirement(&mut self, cutoff: Moment, first_close: bool) {
        let Some(retirement) = &mut self.acquisition_retirement else { return };
        retirement.cutoff = Some(retirement.cutoff.map_or(cutoff, |original| original.min(cutoff)));
        if retirement.failed && first_close {
            retirement.failed = false;
            retirement.retry_on_close = false;
        } else if let Some(job) = &retirement.job
            && job.spec().deadline.is_some_and(|deadline| deadline > cutoff)
        {
            retirement.retry_on_close = true;
            job.cancel();
        }
    }
}
