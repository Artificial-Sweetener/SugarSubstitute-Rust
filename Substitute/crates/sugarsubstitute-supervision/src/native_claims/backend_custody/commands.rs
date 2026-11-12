// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt authenticated one-command outputs before returning matching receipts.
use super::{entry::Entry, registration::Registration, source::State};
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
};
use sugarsubstitute_contracts::{
    backend_custody::{
        BackendCustodyCommand as Command, BackendCustodyFrame, BackendCustodyReceipt as Receipt,
    },
    execution::CounterDeadline,
};
use sugarsubstitute_execution::backend_custody::{
    BackendCustodyMemory, BackendCustodyProcessWatch, BackendCustodyWake,
};
use windows::Win32::{
    Foundation::HANDLE,
    System::{
        JobObjects::{
            IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            QueryInformationJobObject,
        },
        Threading::GetCurrentProcess,
    },
};

impl State {
    /// Resolve the same authenticated installation/account without opening its coordinator lock.
    pub(super) fn initialize(&mut self, memory: &BackendCustodyMemory) -> Result<(), ClaimError> {
        if self.initialized {
            return Err(ClaimError::invalid("custody_source_reinitialized"));
        }
        let resolved = crate::durable_storage::protected::account::resolve_namespace(
            &self.installation,
            &mut self.handles,
            Budget::CustodyCore(self.control.clone()),
        )?;
        let key = memory.key().ok_or_else(|| ClaimError::invalid("custody_source_key"))?;
        if resolved.namespace != key.owner().namespace {
            return Err(ClaimError::invalid("custody_source_namespace"));
        }
        self.directory = resolved.directory;
        self.sid = resolved.account_sid;
        self.initialized = true;
        Ok(())
    }
    /// Apply one complete actual-session request; exact duplicate bytes return the same receipt.
    pub(super) fn apply(
        &mut self,
        frame: BackendCustodyFrame,
        memory: &BackendCustodyMemory,
        wake: &BackendCustodyWake,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<Option<Receipt>, ClaimError> {
        if Some(frame.key) != memory.key() || !self.initialized {
            return Err(ClaimError::invalid("custody_command_session"));
        }
        if let Some(previous) = self.latest {
            if previous == frame {
                return Ok(self.reply);
            }
            if frame.nonce <= previous.nonce {
                return Err(ClaimError::invalid("custody_command_replayed"));
            }
        }
        if self.stopping {
            return Err(ClaimError::invalid("custody_command_stopping"));
        }
        self.latest = Some(frame);
        self.reply = None;
        self.reply_until = None;
        if let Some(parent) = &mut self.parent {
            parent
                .observe_parent_progress()
                .map_err(|cause| ClaimError::new("custody_parent_progress", cause))?;
        }
        let result = match frame.command {
            Command::Heartbeat => {
                self.require_established()?;
                self.reply_until = Some(
                    memory
                        .check_registration()
                        .map_err(|cause| ClaimError::new("custody_heartbeat_clock", cause))?
                        .checked_add(std::time::Duration::from_secs(2))
                        .map_err(|cause| ClaimError::new("custody_heartbeat_deadline", cause))?,
                );
                Receipt::Heartbeat
            }
            Command::ProfilePrepare { .. }
            | Command::ProfilePublished { .. }
            | Command::ProfileRoot { .. }
            | Command::ProfileStop { .. } => {
                return self.apply_profile(frame, memory, wake, context);
            }
            Command::Establish { parent, pid, birth, containment, trial } => {
                if self.parent.is_some() || self.containment.is_some() {
                    return Err(ClaimError::invalid("custody_establish_repeated"));
                }
                self.adopt_transfer(0, parent)?;
                self.adopt_transfer(1, containment)?;
                self.containment = self.transfers[1].take();
                if memory.parent_identity() != Some((pid, birth)) {
                    return Err(ClaimError::invalid("custody_parent_identity"));
                }
                self.validate_containment()?;
                let parent = self.transfer(0)?;
                self.parent = Some(BackendCustodyProcessWatch::parent(parent, memory));
                self.parent
                    .as_mut()
                    .ok_or_else(|| ClaimError::invalid("custody_parent_owner"))?
                    .activate(wake.clone())
                    .map_err(|cause| ClaimError::new("custody_parent_observation", cause))?;
                self.parent
                    .as_mut()
                    .ok_or_else(|| ClaimError::invalid("custody_parent_owner"))?
                    .observe_parent_progress()
                    .map_err(|cause| ClaimError::new("custody_parent_progress", cause))?;
                if let Some(trial) = trial {
                    self.adopt_transfer(0, trial.run)?;
                    self.adopt_transfer(1, trial.snapshot)?;
                    let counter = CounterDeadline::new(trial.until_ticks, trial.frequency)
                        .map_err(|cause| ClaimError::new("installation_witness_counter", cause))?;
                    let until = memory
                        .decode_counter(counter, false)
                        .map_err(|cause| ClaimError::new("installation_witness_cutoff", cause))?;
                    let handles = [self.transfer(0)?, self.transfer(1)?];
                    self.trial.admit(
                        handles,
                        until,
                        std::sync::Arc::new(super::installation::NativeWake(wake.clone())),
                    )?;
                    self.reply_until = Some(until);
                    return Ok(None);
                }
                self.established = true;
                Receipt::Established
            }
            Command::Prepare { resource, job, until_ticks, frequency, .. } => {
                self.require_established()?;
                self.adopt_transfer(0, job)?;
                if self
                    .registrations
                    .iter()
                    .flatten()
                    .any(|entry| entry.resource() == Some(resource))
                {
                    return Err(ClaimError::invalid("custody_resource_repeated"));
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
                let source = CounterDeadline::new(until_ticks, frequency)
                    .map_err(|cause| ClaimError::new("custody_prepare_counter", cause))?;
                let until = memory
                    .decode_counter(source, false)
                    .map_err(|cause| ClaimError::new("custody_prepare_cutoff", cause))?;
                self.reply_until = Some(until);
                self.registrations[index] = Some(Entry::Backend(Box::new(Registration::new(
                    slot,
                    &self.directory,
                    &self.sid,
                    frame.key,
                    &frame.command,
                    until,
                )?)));
                let input = self.transfer(0)?;
                let Entry::Backend(entry) = self.registrations[index]
                    .as_mut()
                    .ok_or_else(|| ClaimError::invalid("custody_registration_owner"))?
                else {
                    return Err(ClaimError::invalid("custody_registration_purpose"));
                };
                entry.prepare(input)?;
                Receipt::Prepared(resource)
            }
            Command::Root { resource, process, .. } => {
                self.require_established()?;
                self.adopt_transfer(0, process)?;
                let index = self
                    .registrations
                    .iter()
                    .position(|entry| {
                        entry.as_ref().is_some_and(|entry| entry.resource() == Some(resource))
                    })
                    .ok_or_else(|| ClaimError::invalid("custody_root_resource"))?;
                let Entry::Backend(entry) = self.registrations[index]
                    .as_ref()
                    .ok_or_else(|| ClaimError::invalid("custody_registration_owner"))?
                else {
                    return Err(ClaimError::invalid("custody_registration_purpose"));
                };
                entry.validate_root(&frame.command)?;
                self.reply_until = Some(entry.original_until());
                let input = self.transfer(0)?;
                let Entry::Backend(entry) = self.registrations[index]
                    .as_mut()
                    .ok_or_else(|| ClaimError::invalid("custody_registration_owner"))?
                else {
                    return Err(ClaimError::invalid("custody_registration_purpose"));
                };
                entry.root(&frame.command, input, wake)?;
                Receipt::Root(resource)
            }
            Command::Stop { resource, until_ticks, frequency } => {
                self.require_established()?;
                let source = CounterDeadline::new(until_ticks, frequency)
                    .map_err(|cause| ClaimError::new("custody_stop_counter", cause))?;
                let until = memory
                    .decode_counter(source, true)
                    .map_err(|cause| ClaimError::new("custody_stop_cutoff", cause))?;
                let entry = self
                    .registrations
                    .iter_mut()
                    .flatten()
                    .find(|entry| entry.resource() == Some(resource))
                    .ok_or_else(|| ClaimError::invalid("custody_stop_resource"))?;
                let Entry::Backend(entry) = entry else {
                    return Err(ClaimError::invalid("custody_registration_purpose"));
                };
                entry.stop(until)?;
                self.reply_until = Some(entry.cleanup_until()?);
                // The driver returns Released only after later complete actual settlement.
                return Ok(None);
            }
            Command::Finish { until_ticks, frequency } => {
                self.require_established()?;
                let source = CounterDeadline::new(until_ticks, frequency)
                    .map_err(|cause| ClaimError::new("custody_finish_counter", cause))?;
                let until = memory
                    .decode_counter(source, true)
                    .map_err(|cause| ClaimError::new("custody_finish_cutoff", cause))?;
                self.stop(until)?;
                self.reply_until = Some(self.control.cleanup(until)?.until());
                self.finish_authorized = true;
                return Ok(None);
            }
            Command::Hello { .. } | Command::Receipt(_) => {
                return Err(ClaimError::invalid("custody_command_phase"));
            }
        };
        self.reply = Some(result);
        Ok(Some(result))
    }
    /// Remove a settled registration only after its actual Released write returned.
    /// Exact last-frame replay remains available without another native effect.
    pub(super) fn receipt_written(&mut self, nonce: u64) -> Result<(), ClaimError> {
        if self.latest.is_none_or(|frame| frame.nonce != nonce) {
            return Err(ClaimError::invalid("custody_written_nonce"));
        }
        if let Some(Receipt::Released(resource)) = self.reply {
            let index = self.registrations.iter().position(|entry| {
                entry.as_ref().is_some_and(|entry| entry.resource() == Some(resource))
            });
            if let Some(index) = index {
                if !self.registrations[index].as_ref().is_some_and(|entry| entry.settled()) {
                    return Err(ClaimError::invalid("custody_written_unsettled"));
                }
                self.registrations[index] = None;
            }
        }
        if let Some(Receipt::ProfileReleased(profile)) = self.reply
            && let Some(index) = self.registrations.iter().position(|entry| {
                entry.as_ref().is_some_and(|entry| entry.profile() == Some(profile))
            })
        {
            if !self.registrations[index].as_ref().is_some_and(|entry| entry.settled()) {
                return Err(ClaimError::invalid("profile_custody_written_unsettled"));
            }
            self.registrations[index] = None;
        }
        Ok(())
    }
    /// Require the actual established parent and occupied self-containment cells.
    pub(super) fn require_established(&self) -> Result<(), ClaimError> {
        if self.established
            && self.parent.is_some()
            && self.containment.as_ref().is_some_and(Handle::is_armed)
        {
            Ok(())
        } else {
            Err(ClaimError::invalid("custody_not_established"))
        }
    }
    /// Adopt the actual destination output into funded storage before any fallible query.
    pub(super) fn adopt_transfer(&mut self, index: usize, raw: u64) -> Result<(), ClaimError> {
        let slot = self
            .transfers
            .get_mut(index)
            .ok_or_else(|| ClaimError::invalid("custody_transfer_index"))?;
        if slot.is_some() {
            return Err(ClaimError::invalid("custody_transfer_pending"));
        }
        *slot = Some(Handle::new(
            HANDLE(raw as usize as *mut _),
            Budget::CustodyCore(self.control.clone()),
        ));
        Ok(())
    }
    /// Move the identical output directly to the next already-funded durable owner.
    pub(super) fn transfer(
        &mut self,
        index: usize,
    ) -> Result<std::os::windows::io::OwnedHandle, ClaimError> {
        let slot = self
            .transfers
            .get_mut(index)
            .ok_or_else(|| ClaimError::invalid("custody_transfer_index"))?;
        let output = slot
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("custody_transfer_missing"))?
            .take_custody_output()?;
        *slot = None;
        Ok(output)
    }
    /// Verify this exact adopted Job contains the actual current helper process.
    fn validate_containment(&self) -> Result<(), ClaimError> {
        let job = self
            .containment
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("custody_containment_owner"))?
            .raw();
        let mut member = windows::core::BOOL::default();
        self.control.check()?;
        // SAFETY: The actual Job output remains durably owned; the current-process
        // pseudo handle is borrowed, and the initialized membership result is checked.
        unsafe { IsProcessInJob(GetCurrentProcess(), Some(job), &mut member) }
            .map_err(|cause| ClaimError::new("custody_self_membership", cause))?;
        if !member.as_bool() {
            return Err(ClaimError::invalid("custody_self_membership"));
        }
        self.control.check()?;
        let parent = self.transfers[0]
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("custody_parent_output"))?
            .raw();
        let mut parent_member = windows::core::BOOL::default();
        // SAFETY: Both exact adopted outputs remain durably owned. An enclosing
        // coordinator Job must not include this independently contained witness.
        unsafe { IsProcessInJob(parent, Some(job), &mut parent_member) }
            .map_err(|cause| ClaimError::new("custody_parent_containment", cause))?;
        if parent_member.as_bool() {
            return Err(ClaimError::invalid("custody_not_independent"));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        self.control.check()?;
        // SAFETY: Retained Job and complete initialized output; this query grants no new output.
        unsafe {
            QueryInformationJobObject(
                Some(job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of_val(&limits) as u32,
                None,
            )
        }
        .map_err(|cause| ClaimError::new("custody_self_limits", cause))?;
        if !limits.BasicLimitInformation.LimitFlags.contains(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE) {
            return Err(ClaimError::invalid("custody_self_containment"));
        }
        Ok(())
    }
}
