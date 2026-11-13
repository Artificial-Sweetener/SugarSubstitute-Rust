// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Re-elect only after a sealed unsubmitted source returns with original custody.
use super::*;
impl CoordinatorEntryOwner {
    /// Restore the unchanged source after successful native client retirement.
    /// No election proof is granted; the next finite job reacquires both exclusions.
    /// # Errors
    /// Leaves the caller's complete handback intact on closed, foreign, or expired input.
    pub fn recover_unsubmitted_client(
        &mut self,
        returned: &mut Option<sugarsubstitute_execution::UnsubmittedEntryRequest>,
    ) -> Result<(), EntryAdmissionError> {
        self.current()?;
        if !matches!(self.phase, EntryPhase::Candidate | EntryPhase::Contended)
            || self.pending.is_some()
            || self.request.is_some()
            || self.election.is_some()
            || self.coordinator.is_some()
        {
            return Err(EntryAdmissionError::Identity);
        }
        let returned_source = returned.as_ref().ok_or(EntryAdmissionError::Identity)?;
        returned_source
            .validate(
                &self.discovered.ok_or(EntryAdmissionError::Identity)?.facts,
                self.candidate,
                self.source_setup.ok_or(EntryAdmissionError::Identity)?,
                self.client.as_ref().ok_or(EntryAdmissionError::Identity)?,
                self.budget,
            )
            .map_err(EntryAdmissionError::PreparedTransport)?;
        let source = returned.take().ok_or(EntryAdmissionError::Identity)?;
        self.request = Some(source.into_request());
        self.candidate = None;
        self.phase = EntryPhase::Discovered;
        Ok(())
    }
}
