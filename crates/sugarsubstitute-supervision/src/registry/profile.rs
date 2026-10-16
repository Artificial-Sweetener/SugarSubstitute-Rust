// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the exact desktop family for off-UI browser qualification.
use super::*;
use sugarsubstitute_contracts::lifecycle::RunKey;
pub(crate) mod prepared;
mod recovery;

pub(crate) struct ProfileFamily {
    pub(crate) identity: RegistryIdentity,
    pub(crate) ticket: LaunchTicket,
    proof: super::family_proof::RetainedFamilyProof,
}
impl WindowsProcessRegistry {
    pub(crate) fn retained_desktop(
        &self,
        run: RunKey,
    ) -> Result<Option<RegistrySnapshot>, RegistryAdmissionError> {
        if run.owner != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        Ok(self.records.iter().flatten().find_map(|record| {
            (record.ticket.binding == FamilyBinding::Desktop(run)).then(|| record.snapshot())
        }))
    }
    pub(crate) fn profile_family(
        &self,
        ticket: LaunchTicket,
        run: RunKey,
    ) -> Result<ProfileFamily, RegistryAdmissionError> {
        if run.owner != self.owner || ticket.binding != FamilyBinding::Desktop(run) {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        let proof = self.retained_family(ticket)?;
        Ok(ProfileFamily { identity: proof.identity.clone(), ticket: proof.ticket, proof })
    }
}
impl ProfileFamily {
    /// Observe the same retained admission fence during UI authorization.
    pub(crate) fn is_live(&self) -> bool {
        self.proof.is_live()
    }
    /// Verify the actual browser against this desktop's current retained Job.
    pub(crate) fn qualify_browser(
        &self,
        pid: u32,
        context: &JobContext,
    ) -> Result<Identity, RegistryError> {
        self.proof.qualify_member(pid, context)
    }
}

#[cfg(test)]
mod tests;
