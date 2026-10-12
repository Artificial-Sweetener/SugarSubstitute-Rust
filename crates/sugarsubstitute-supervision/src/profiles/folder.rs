// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare each main attempt's path under its own retained finite-operation charge.
use super::*;
use sugarsubstitute_contracts::startup::MainIdentity;

impl WindowsProfileLease {
    /// Copy actual held canonical folder facts for this exact active main attempt.
    /// The finite worker claims the full bounded buffer before allocating. Each retained
    /// result keeps its own charge, including results invalidated by later retirement.
    /// # Errors
    /// Rejects foreign/repeated attempts, expired or overloaded execution, and unheld custody.
    pub fn submit_folder_preparation(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        identity: MainIdentity,
    ) -> Result<JobId, ClaimAdmissionError> {
        self.validate_submission(scope, spec)?;
        if !self.is_held() {
            return Err(ClaimAdmissionError::Busy);
        }
        let binding = self.attempts.admit(
            identity,
            spec.deadline.ok_or(ClaimAdmissionError::Specification)?,
            attempt::ProofKind::Folder,
        )?;
        self.submit(
            execution,
            scope,
            spec,
            move |state, context| {
                storage::check(context)?;
                if !binding.live(identity) || !state.held {
                    return Err(ClaimError::invalid("profile_folder_retired_attempt"));
                }
                let canonical = state
                    .folder
                    .as_ref()
                    .ok_or_else(|| ClaimError::invalid("profile_folder_missing"))?;
                if canonical.is_empty() || canonical.len() > 32766 || canonical.contains(&0) {
                    return Err(ClaimError::invalid("profile_folder_shape"));
                }
                let memory = context
                    .memory()
                    .claim(65532 + 256)
                    .map_err(|e| ClaimError::new("profile_folder_copy_claim", e))?;
                let mut units = Vec::new();
                units
                    .try_reserve_exact(canonical.len())
                    .map_err(|e| ClaimError::new("profile_folder_copy_allocation", e))?;
                if units.capacity() > 32766 {
                    return Err(ClaimError::invalid("profile_folder_copy_capacity"));
                }
                units.extend_from_slice(canonical);
                storage::check(context)?;
                state.prepared_folder =
                    Some(storage::PreparedFolder { binding, job: context.job_id(), units, memory });
                Ok(())
            },
            false,
        )?;
        self.pending_attempt = Some(identity);
        Ok(spec.id)
    }
}
