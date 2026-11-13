// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route original profile observations through the one live Parent consumer.
use super::*;
use sugarsubstitute_supervision::profiles::{ProfileCompletion, WindowsProfileLease};

impl ParentLifecycle {
    /// Route Main proofs by their original request; independently drain the lease's
    /// admitted release after closing, including its original helper response.
    pub(in crate::production) fn profile_completion(
        &mut self,
        batch: &CompletionBatch,
        profile: &mut WindowsProfileLease,
    ) -> Result<Option<ProfileCompletion>> {
        if self.profile_pending.is_some() {
            return Ok(profile.consume(batch)?);
        }
        if let Some(completion) = profile.consume_release(self.run, batch)? {
            if let Some(cause) = completion.failure {
                return Err(Box::new(cause));
            }
            if completion.kind != sugarsubstitute_contracts::execution::TerminalKind::Success {
                return Err(ParentError::OwnerFailed.into());
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "profile_observation/tests.rs"]
pub(super) mod tests;
