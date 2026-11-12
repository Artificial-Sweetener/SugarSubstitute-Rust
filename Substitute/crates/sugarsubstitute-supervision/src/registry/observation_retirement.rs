// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Request exact root-watch retirement while retaining the family and actual exit authority.

use super::*;

impl WindowsProcessRegistry {
    /// Request cancellation of this exact root observation, returning its execution identity.
    ///
    /// This request neither stops the process nor closes launch admission. The family,
    /// native handles and original stop budget remain owned. Repeated requests for a
    /// still-retained watch return the same job identity. Its actual typed cancellation
    /// or timeout is reported separately by `RootObservationRetired`; an exit winning
    /// the race remains an actual `RootExited` fact. Cleanup can observe the root again
    /// through its bounded member-watch path.
    ///
    /// # Errors
    /// Rejects foreign/stale keys, finite work still pending and absent/settled watches.
    /// No native state lock, wait, close or process termination occurs here.
    pub fn request_observation_retirement(
        &mut self,
        key: ProcessKey,
    ) -> Result<JobId, RegistryAdmissionError> {
        let index = self.index(key)?;
        let record = self.records[index].as_ref().ok_or(RegistryAdmissionError::Stale)?;
        if record.creation.is_some() || record.cleanup.is_some() {
            return Err(RegistryAdmissionError::Busy);
        }
        let watch = record.root_watch.as_ref().ok_or(RegistryAdmissionError::Closed)?;
        if record.settled {
            return Err(RegistryAdmissionError::Closed);
        }
        let job = watch.spec().id;
        watch.cancel();
        Ok(job)
    }
}
