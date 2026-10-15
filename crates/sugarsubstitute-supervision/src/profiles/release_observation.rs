// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the original release job and remote custody receipt through one consumer.
use super::*;
use sugarsubstitute_contracts::execution::{JobClass, Operation};

impl WindowsProfileLease {
    /// Consume only this run's admitted profile release and its original helper response.
    /// Observation continues after the finite job completes so the same ProfileReleased
    /// receipt can unblock the final checked close operation. No cutoff is sampled here.
    /// # Errors
    /// Rejects another run or a pending acquisition/Main proof without consuming it.
    /// Preserves actual worker and transport failures through the existing completion owner.
    pub fn consume_release(
        &mut self,
        run: RunKey,
        batch: &CompletionBatch,
    ) -> Result<Option<ProfileCompletion>, ClaimError> {
        if run != self.run {
            return Err(ClaimError::invalid("profile_release_observation_run"));
        }
        let Some(until) = self.release_deadline else {
            return if self.pending.is_some() {
                Err(ClaimError::invalid("profile_release_observation_purpose"))
            } else {
                Ok(None)
            };
        };
        if !self.shared.closing.load(Ordering::Acquire)
            || self.pending_attempt.is_some()
            || self.pending_browser
            || self.pending.as_ref().is_some_and(|job| {
                let spec = job.spec();
                spec.operation != Operation::NativeLifecycle
                    || spec.class != JobClass::InfrastructureLifecycle
                    || spec.deadline != Some(until)
                    || spec.replacement.is_some()
                    || spec.reservation_bytes < PROFILE_OPERATION_BYTES
            })
        {
            return Err(ClaimError::invalid("profile_release_observation_purpose"));
        }
        self.consume(batch)
    }
}
