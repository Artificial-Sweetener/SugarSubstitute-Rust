// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Move original environment custody to the existing native worker once callbacks return.

use super::{EntryPreparationInputError, EntryPreparationOwner, slot::PreparationSlot};
use crate::{
    admission::{Core, Retirement},
    scope::lock,
    task::ErasedJob,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobId, JobPhase, PanicCause};

/// The passive slot and job aliases drop before the retained original whole owner.
pub(crate) struct EntryPreparationRetirement {
    slot: Arc<PreparationSlot>,
    job: Arc<dyn ErasedJob>,
    owner: EntryPreparationOwner,
    completed: bool,
}
impl EntryPreparationRetirement {
    pub(crate) fn job_id(&self) -> JobId {
        self.slot.job
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.owner.fail_owner(cause);
    }

    /// The owner's once-attempted Free runs outside Core and passive-slot locks.
    /// An uncertain Free remains owned; another worker wake cannot retry it.
    pub(crate) fn progress(&mut self, core: &Core) {
        if self.completed {
            return;
        }
        self.owner.progress_retirement();
        if !self.owner.retirement_settled() {
            return;
        }
        let dispose = {
            let _state = lock(&core.state);
            let mut slot = lock(&self.slot.state);
            slot.native_handoff = false;
            slot.native_retirement_active = false;
            std::mem::take(&mut slot.pending_result_disposal)
        };
        if dispose {
            self.job.dispose();
        }
        core.retire_native_custody(self.slot.job);
        self.completed = true;
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
}

impl Core {
    /// Accept exactly one callback-quiescent original owner before publishing its terminal result.
    /// This admits no job and samples no clock; it transfers existing stopped custody only.
    pub(crate) fn handoff_entry_preparation(
        self: &Arc<Self>,
        owner: EntryPreparationOwner,
    ) -> Result<(), (EntryPreparationInputError, EntryPreparationOwner)> {
        let context = owner.original_context();
        let binding = match owner.original_binding() {
            Ok(binding) => binding,
            Err(cause) => return Err((cause, owner)),
        };
        if context.owner_id() != self.id || !binding.matches_core(self) {
            return Err((EntryPreparationInputError::ForeignOwner, owner));
        }
        let mut state = lock(&self.state);
        let Some(record) = state.jobs.get(&context.job_id()) else {
            drop(state);
            return Err((EntryPreparationInputError::Closed, owner));
        };
        let slot = match owner.passive_slot() {
            Ok(slot) => slot,
            Err(cause) => {
                drop(state);
                return Err((cause, owner));
            }
        };
        if record.scope_id != context.scope_id()
            || record.spec != context.spec()
            || record.entry_setup != Some(binding.setup())
            || record.phase != JobPhase::Running
            || record.retiring
            || record.terminal_published
            || !record.memory.as_ref().is_some_and(|memory| memory.matches_lease(context.memory()))
            || !record.preparation.as_ref().is_some_and(|stored| Arc::ptr_eq(stored, slot))
        {
            drop(state);
            return Err((EntryPreparationInputError::OriginalSetupMismatch, owner));
        }
        let mut passive = lock(&slot.state);
        if !passive.callback_quiescent || passive.native_handoff || passive.native_retirement_active
        {
            drop(passive);
            drop(state);
            return Err((EntryPreparationInputError::Closed, owner));
        }
        passive.native_handoff = true;
        let slot = slot.clone();
        let job = record.job.clone();
        drop(passive);
        state.retired.push_back(Retirement::Environment(EntryPreparationRetirement {
            slot,
            job,
            owner,
            completed: false,
        }));
        drop(state);
        self.changed();
        Ok(())
    }

    /// Native selection never changes the already observed terminal phase or class credit.
    pub(crate) fn take_environment_retirement(&self) -> Option<EntryPreparationRetirement> {
        let mut state = lock(&self.state);
        let index = state
            .retired
            .iter()
            .position(|retired| matches!(retired, Retirement::Environment(_)))?;
        let retired = state.retired.remove(index)?;
        match retired {
            Retirement::Environment(retired) => {
                lock(&retired.slot.state).native_retirement_active = true;
                Some(retired)
            }
            other => {
                state.retired.push_front(other);
                None
            }
        }
    }
}
