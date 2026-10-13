// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Derive close-only parent custody from a real separately admitted cleanup record.

use super::CaptureBundleOrigin;
use super::origin::{CleanupBinding, CleanupLatch};
use crate::{ExecutionClient, scope::lock};
use std::{sync::Arc, time::Duration};
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext, Moment};

/// Retain one sealed target and the actual original cleanup record's ports.
/// This noncloneable proof authorizes checked cleanup only. Acknowledgement,
/// cancellation or a zero output counter never establishes physical settlement.
pub struct CaptureBundleRetirement {
    pub(super) authority: RetirementAuthority,
    pub(super) origin: CaptureBundleOrigin,
    pub(super) cutoff: Moment,
}
pub(super) enum RetirementAuthority {
    Parent(JobContext),
    #[cfg(any(windows, test))]
    Desktop {
        custody: crate::desktop_capture::metadata::DesktopCaptureCustodyRef,
        original: JobContext,
    },
}

impl ExecutionClient {
    /// Begin the target's first close transaction under a real separate cleanup job.
    /// The first claimant alone samples the actual Core clock outside every lock;
    /// the cutoff is immutable, bounded by that record and fifteen seconds.
    /// Original target effect admission may already be terminal or acknowledged.
    ///
    /// # Errors
    /// Rejects foreign targets and mismatched, stopped or expired cleanup records.
    /// Concurrent first observation rejects without waiting. Clock/arithmetic failure
    /// latches permanently and preserves its original cause without renewing cleanup.
    pub fn capture_bundle_retirement(
        &self,
        target: &CaptureBundleOrigin,
        observed: &JobContext,
    ) -> Result<CaptureBundleRetirement, AdmissionError> {
        let origin = target.state()?;
        if !std::sync::Weak::ptr_eq(&origin.core, &Arc::downgrade(&self.core))
            || observed.job_id() == origin.original.job_id()
            || origin.desktop_child
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let (original, binding, sample) = {
            let state = lock(&self.core.state);
            let mut cleanup = lock(&origin.cleanup);
            if let CleanupLatch::ArithmeticFailed(cause) = &*cleanup {
                return Err(AdmissionError::Unexpected(crate::runtime::error(*cause)));
            }
            if matches!(*cleanup, CleanupLatch::ClockFailed) || self.core.clock.failed() {
                drop(cleanup);
                drop(state);
                return match self.core.clock.metadata() {
                    Err(cause) => Err(AdmissionError::Unexpected(cause)),
                    Ok(_) => Err(AdmissionError::InvalidCaptureAdmission),
                };
            }
            // Facts and actual cancellation are checked before calling any provider.
            super::validate::cleanup_facts(&self.core, &state, observed)?;
            let record = super::validate::association(&self.core, &state, observed)?;
            let deadline = record.deadline.ok_or(AdmissionError::DeadlineRequired)?;
            let binding = CleanupBinding { scope: record.scope_id, spec: record.spec, deadline };
            let sample = match &*cleanup {
                CleanupLatch::Dormant => {
                    *cleanup = CleanupLatch::Sampling(binding);
                    true
                }
                CleanupLatch::Latched { binding: original, .. } if *original == binding => false,
                _ => return Err(AdmissionError::InvalidCaptureAdmission),
            };
            let original = super::validate::reconstruct(&self.core, record, observed);
            (original, binding, sample)
        };
        let now = match self.core.clock.metadata() {
            Ok(now) => now,
            Err(cause) => {
                if sample {
                    *lock(&origin.cleanup) = CleanupLatch::ClockFailed;
                }
                return Err(AdmissionError::Unexpected(cause));
            }
        };
        let cutoff = {
            let state = lock(&self.core.state);
            let mut cleanup = lock(&origin.cleanup);
            if sample {
                if !matches!(*cleanup, CleanupLatch::Sampling(current) if current == binding) {
                    return Err(AdmissionError::InvalidCaptureAdmission);
                }
                let limit = match now.checked_add(Duration::from_secs(15)) {
                    Ok(limit) => limit,
                    Err(cause) => {
                        *cleanup = CleanupLatch::ArithmeticFailed(cause);
                        return Err(AdmissionError::Unexpected(crate::runtime::error(cause)));
                    }
                };
                *cleanup = CleanupLatch::Latched { binding, cutoff: limit.min(binding.deadline) };
            }
            let CleanupLatch::Latched { binding: current, cutoff } = &*cleanup else {
                return Err(AdmissionError::InvalidCaptureAdmission);
            };
            if *current != binding {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            super::validate::cleanup(&self.core, &state, &original, now)?;
            if now >= *cutoff {
                return Err(AdmissionError::DeadlineExpired { deadline: *cutoff, now });
            }
            *cutoff
        };
        // All final owning guard destruction remains outside Core and cleanup locks.
        Ok(CaptureBundleRetirement {
            authority: RetirementAuthority::Parent(original),
            origin: target.clone(),
            cutoff,
        })
    }
}

impl CaptureBundleRetirement {
    /// Validate the original close record and immutable target transaction freshly.
    /// No effect validator requiring the original target to remain Running is used.
    ///
    /// # Errors
    /// Preserves actual cancellation, cutoff, record retirement and fatal clock cause.
    pub fn check(&self) -> Result<(), AdmissionError> {
        let origin = self.origin.state()?;
        let core = origin.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&core.state);
        #[cfg(any(windows, test))]
        if let RetirementAuthority::Desktop { custody, .. } = &self.authority {
            let cutoff = super::desktop_retirement::validate(
                &core,
                &state,
                &self.origin,
                custody,
                Some(now),
            )?;
            return if cutoff == self.cutoff {
                Ok(())
            } else {
                Err(AdmissionError::InvalidCaptureAdmission)
            };
        }
        let original = self.original_context();
        super::validate::cleanup(&core, &state, original, now)?;
        let cleanup = lock(&origin.cleanup);
        let binding = CleanupBinding {
            scope: original.scope_id(),
            spec: original.spec(),
            deadline: original.deadline().ok_or(AdmissionError::DeadlineRequired)?,
        };
        if !matches!(*cleanup, CleanupLatch::Latched { binding: current, cutoff }
            if current == binding && cutoff == self.cutoff)
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        if now >= self.cutoff {
            return Err(AdmissionError::DeadlineExpired { deadline: self.cutoff, now });
        }
        Ok(())
    }
    /// Borrow only the actual original cleanup record's reconstructed Core ports.
    pub const fn original_context(&self) -> &JobContext {
        match &self.authority {
            RetirementAuthority::Parent(original) => original,
            #[cfg(any(windows, test))]
            RetirementAuthority::Desktop { original, .. } => original,
        }
    }
    /// Return the first immutable cleanup cutoff; this fact grants no authority.
    pub const fn cleanup_cutoff(&self) -> Moment {
        self.cutoff
    }
    /// Compare the exact sealed origin allocation without callbacks or native effects.
    pub fn matches_origin(&self, target: &CaptureBundleOrigin) -> bool {
        self.origin.same_allocation(target)
    }
}
