// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt genuine profile outputs and publish receipts through the original shared custody table.
use super::{entry::Entry, profile_registration::ProfileRegistration, source::State};
use crate::ClaimError;
use sugarsubstitute_contracts::{
    backend_custody::{
        BackendCustodyCommand as Command, BackendCustodyFrame, BackendCustodyReceipt as Receipt,
    },
    execution::{CounterDeadline, JobContext},
};
use sugarsubstitute_execution::backend_custody::{BackendCustodyMemory, BackendCustodyWake};
impl State {
    /// Apply only profile-purpose frames already admitted by the authentic session owner.
    pub(super) fn apply_profile(
        &mut self,
        frame: BackendCustodyFrame,
        memory: &BackendCustodyMemory,
        wake: &BackendCustodyWake,
        context: &JobContext,
    ) -> Result<Option<Receipt>, ClaimError> {
        let result = match frame.command {
            Command::ProfilePrepare { profile, job, directory, until_ticks, frequency, .. } => {
                self.require_established()?;
                self.adopt_transfer(0, job)?;
                self.adopt_transfer(1, directory)?;
                if self.registrations.iter().flatten().any(|entry| entry.profile() == Some(profile))
                {
                    return Err(ClaimError::invalid("profile_custody_repeated"));
                }
                let index = self
                    .registrations
                    .iter()
                    .position(Option::is_none)
                    .ok_or_else(|| ClaimError::invalid("custody_registration_capacity"))?;
                let slot = self
                    .pool
                    .acquire()?
                    .ok_or_else(|| ClaimError::invalid("custody_registration_capacity"))?;
                let until = memory
                    .decode_counter(
                        CounterDeadline::new(until_ticks, frequency)
                            .map_err(|cause| ClaimError::new("profile_custody_counter", cause))?,
                        false,
                    )
                    .map_err(|cause| ClaimError::new("profile_custody_cutoff", cause))?;
                self.reply_until = Some(until);
                self.registrations[index] = Some(Entry::Profile(Box::new(
                    ProfileRegistration::new(slot, &self.sid, frame.key, &frame.command, until)?,
                )));
                let job = self.transfer(0)?;
                let directory = self.transfer(1)?;
                let Entry::Profile(entry) = self.registrations[index]
                    .as_mut()
                    .ok_or_else(|| ClaimError::invalid("profile_custody_owner"))?
                else {
                    return Err(ClaimError::invalid("profile_custody_purpose"));
                };
                entry.prepare(job, directory)?;
                Receipt::ProfilePrepared(profile)
            }
            Command::ProfilePublished { profile } => {
                self.require_established()?;
                let entry = self
                    .registrations
                    .iter_mut()
                    .flatten()
                    .find(|entry| entry.profile() == Some(profile))
                    .ok_or_else(|| ClaimError::invalid("profile_custody_owner"))?;
                let Entry::Profile(entry) = entry else {
                    return Err(ClaimError::invalid("profile_custody_purpose"));
                };
                self.reply_until = Some(entry.original_until());
                entry.published(profile, context)?;
                Receipt::ProfilePublished(profile)
            }
            Command::ProfileRoot { profile, process, .. } => {
                self.require_established()?;
                self.adopt_transfer(0, process)?;
                let index = self
                    .registrations
                    .iter()
                    .position(|entry| {
                        entry.as_ref().is_some_and(|entry| entry.profile() == Some(profile))
                    })
                    .ok_or_else(|| ClaimError::invalid("profile_custody_owner"))?;
                let input = self.transfer(0)?;
                let Entry::Profile(entry) = self.registrations[index]
                    .as_mut()
                    .ok_or_else(|| ClaimError::invalid("profile_custody_owner"))?
                else {
                    return Err(ClaimError::invalid("profile_custody_purpose"));
                };
                self.reply_until = Some(entry.original_until());
                entry.root(&frame.command, input, wake)?;
                Receipt::ProfileRoot(profile)
            }
            Command::ProfileStop { profile, until_ticks, frequency } => {
                self.require_established()?;
                let until = memory
                    .decode_counter(
                        CounterDeadline::new(until_ticks, frequency).map_err(|cause| {
                            ClaimError::new("profile_custody_stop_counter", cause)
                        })?,
                        true,
                    )
                    .map_err(|cause| ClaimError::new("profile_custody_stop_cutoff", cause))?;
                let entry = self
                    .registrations
                    .iter_mut()
                    .flatten()
                    .find(|entry| entry.profile() == Some(profile))
                    .ok_or_else(|| ClaimError::invalid("profile_custody_owner"))?;
                entry.stop(until)?;
                self.reply_until = Some(until);
                return Ok(None);
            }
            _ => return Err(ClaimError::invalid("profile_custody_command_purpose")),
        };
        self.reply = Some(result);
        Ok(self.reply)
    }
}
