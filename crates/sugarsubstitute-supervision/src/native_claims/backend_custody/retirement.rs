// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep every unresolved native family while progressing independent owned cleanup.
use super::source::State;
use crate::ClaimError;
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyCommand, BackendCustodyReceipt},
    execution::{JobContext, Moment},
};
use sugarsubstitute_execution::backend_custody::{BackendCustodyMemory, BackendCustodyWake};

impl State {
    /// Preserve one first and at most eight secondary real causes under the fixed metadata.
    pub(super) fn fail(&mut self, cause: ClaimError) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if let Some(slot) = self.secondary.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omissions = self.omissions.saturating_add(1);
        }
        self.failed = true;
    }
    /// Close admission once and latch the same original helper/registration cutoffs.
    pub(super) fn stop(&mut self, until: Moment) -> Result<(), ClaimError> {
        if self.stopping {
            return Ok(());
        }
        self.stopping = true;
        let proof = self.control.cleanup(until)?;
        for entry in self.registrations.iter_mut().flatten() {
            entry.stop(proof.until())?;
        }
        Ok(())
    }
    /// Parent death and actual root waits select cleanup; neither proves Released.
    pub(super) fn progress(
        &mut self,
        context: &JobContext,
        _memory: &BackendCustodyMemory,
        wake: &BackendCustodyWake,
    ) -> Result<bool, ClaimError> {
        if self.parent.as_ref().is_some_and(|parent| parent.signaled()) {
            self.stop(Moment::from_nanos(u64::MAX))?;
        }
        let mut failures: [Option<ClaimError>; 8] = std::array::from_fn(|_| None);
        let mut failure = None;
        for entry in self.registrations.iter_mut().flatten() {
            if !entry.cleaning() && entry.root_signaled() {
                entry.stop(Moment::from_nanos(u64::MAX))?;
            }
            if !entry.cleaning() || entry.settled() {
                continue;
            }
            if let Err(cause) = entry.retire(context, wake) {
                if failure.is_none() {
                    failure = Some(cause);
                } else if let Some(slot) = failures.iter_mut().find(|slot| slot.is_none()) {
                    *slot = Some(cause);
                } else {
                    self.omissions = self.omissions.saturating_add(1);
                }
            }
        }
        if let Some(cause) = failure {
            self.fail(cause.clone());
            for secondary in failures.into_iter().flatten() {
                self.fail(secondary);
            }
            return Err(cause);
        }
        if let Some(frame) = self.latest
            && let BackendCustodyCommand::ProfileStop { profile, .. } = frame.command
            && self
                .registrations
                .iter()
                .flatten()
                .any(|entry| entry.profile() == Some(profile) && entry.settled())
        {
            self.reply = Some(BackendCustodyReceipt::ProfileReleased(profile));
        }
        if let Some(frame) = self.latest
            && let BackendCustodyCommand::Stop { resource, .. } = frame.command
            && self
                .registrations
                .iter()
                .flatten()
                .any(|entry| entry.resource() == Some(resource) && entry.settled())
        {
            self.reply = Some(BackendCustodyReceipt::Released(resource));
        }
        if !self.stopping || self.registrations.iter().flatten().any(|entry| !entry.settled()) {
            return Ok(false);
        }
        let proof = self.control.cleanup(Moment::from_nanos(u64::MAX))?;
        if let Some(parent) = &mut self.parent {
            parent
                .retire(&proof)
                .map_err(|cause| ClaimError::new("custody_parent_retire", cause))?;
        }
        self.parent = None;
        for output in self.transfers.iter_mut().flatten() {
            output.close()?;
        }
        self.transfers = [None, None];
        for output in self.handles.iter_mut().rev() {
            output.close()?;
        }
        self.handles.clear();
        self.settled = true;
        if self
            .latest
            .is_some_and(|frame| matches!(frame.command, BackendCustodyCommand::Finish { .. }))
        {
            self.reply = Some(BackendCustodyReceipt::Finished);
        }
        // The verified self-containment is an explicit process-lifetime capsule;
        // its original metadata remains held through actual headless process exit.
        Ok(true)
    }
}
