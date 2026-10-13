// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Seal actual original preparation identity and preserve its controls after terminal ACK.

use super::{EntryPreparationControlCause, EntryPreparationInputError, slot::PreparationSlot};
use crate::{
    admission::{Core, State},
    scope::lock,
};
use std::sync::{Arc, Mutex, Weak};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{
        AdmittedJobResources, CancelControl, JobClass, JobContext, JobPhase, MemoryClaim, Moment,
        Operation, TimeError,
    },
};

pub(crate) const PREPARATION_BYTES: u64 = 4 * 1024 * 1024;

/// Constructor-private proof of the genuine preparation record and original ports.
/// This lives in the outer charged owner, never in Core or its passive slot.
pub(crate) struct OriginalPreparation {
    core: Weak<Core>,
    context: JobContext,
    setup: EntrySetupBudget,
    sampled: Mutex<Sampled>,
}
struct Sampled {
    last: Moment,
    regression: bool,
    publication: Publication,
}

/// Permit one public validation at a time and close permanently on failure or unwind.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Publication {
    Open,
    Checking,
    Closed,
}

impl OriginalPreparation {
    /// Revalidate supplied association facts against an actual Running record.
    /// This constructor samples no provider. The caller adopts the returned full
    /// claim before metadata allocation and performs check before the first effect.
    /// Caller clock/cancellation objects are neither compared nor invoked.
    pub(crate) fn mint(
        core: &Arc<Core>,
        observed: &JobContext,
        setup: EntrySetupBudget,
    ) -> Result<(Self, MemoryClaim), EntryPreparationInputError> {
        if observed.owner_id() != core.id {
            return Err(EntryPreparationInputError::ForeignOwner);
        }
        let now = setup.started();
        let context = {
            let mut state = lock(&core.state);
            let record =
                state.jobs.get(&observed.job_id()).ok_or(EntryPreparationInputError::Closed)?;
            if record.entry_setup != Some(setup)
                || record.spec != observed.spec()
                || record.scope_id != observed.scope_id()
                || record.deadline != observed.deadline()
                || record.phase != JobPhase::Running
                || record.retiring
                || record.spec.class != JobClass::InfrastructureLifecycle
                || record.spec.operation != Operation::NativeLifecycle
                || record.spec.reservation_bytes != PREPARATION_BYTES
                || !record
                    .memory
                    .as_ref()
                    .is_some_and(|memory| memory.matches_lease(observed.memory()))
                || record.preparation_issued
            {
                return Err(EntryPreparationInputError::OriginalSetupMismatch);
            }
            let context = JobContext::new(
                core.id,
                record.scope_id,
                record.spec,
                record.deadline,
                record.job.cancellation(),
                core.clock.clone(),
                AdmittedJobResources::new(observed.memory().clone(), None),
            );
            live(&state, &context, setup, now)?;
            if let Some(record) = state.jobs.get_mut(&observed.job_id()) {
                record.preparation_issued = true;
            }
            context
        };
        let memory =
            context.memory().claim(PREPARATION_BYTES).map_err(EntryPreparationInputError::Claim)?;
        Ok((
            Self {
                core: Arc::downgrade(core),
                context,
                setup,
                sampled: Mutex::new(Sampled {
                    last: now,
                    regression: false,
                    publication: Publication::Open,
                }),
            },
            memory,
        ))
    }

    /// Borrow the genuine reconstructed ports, not caller-supplied public ports.
    pub(crate) fn context(&self) -> &JobContext {
        &self.context
    }
    pub(crate) fn setup(&self) -> EntrySetupBudget {
        self.setup
    }
    pub(crate) fn matches_core(&self, core: &Core) -> bool {
        std::ptr::eq(self.core.as_ptr(), core)
    }

    /// Read the last adopted original sample without calling a provider.
    /// A runner uses its genuine dispatch sample as a lower bound if the first
    /// control check failed before adopting a later sample. This never renews a cutoff.
    pub(crate) fn last_observed(&self) -> Moment {
        lock(&self.sampled).last
    }

    /// Stop fresh publication without suppressing mandatory post-effect control observations.
    pub(crate) fn close_publication(&self) {
        lock(&self.sampled).publication = Publication::Closed;
    }

    /// Validate one fresh publication without allowing repeated retained fatal allocations.
    /// Reentrant or concurrent attempts fail before sampling a provider. An unwind leaves
    /// permission closed; cleanup still uses check to preserve genuine later controls.
    pub(crate) fn check_publication(
        &self,
        core: &Core,
        setup: EntrySetupBudget,
    ) -> Result<Moment, EntryPreparationInputError> {
        if !self.matches_core(core) {
            return Err(EntryPreparationInputError::ForeignOwner);
        }
        if setup != self.setup {
            return Err(EntryPreparationInputError::OriginalSetupMismatch);
        }
        {
            let mut sampled = lock(&self.sampled);
            if sampled.publication != Publication::Open {
                return Err(EntryPreparationInputError::Closed);
            }
            sampled.publication = Publication::Checking;
        }
        let result = self.check(core, setup);
        let mut sampled = lock(&self.sampled);
        match result {
            Ok(now) if sampled.publication == Publication::Checking => {
                sampled.publication = Publication::Open;
                Ok(now)
            }
            Ok(_) => Err(EntryPreparationInputError::Closed),
            Err(cause) => {
                sampled.publication = Publication::Closed;
                Err(cause)
            }
        }
    }

    /// Sample the genuine provider outside every owned lock, then check current ancestry.
    /// Removal of an acknowledged record alone does not revoke delivered source custody.
    pub(crate) fn check(
        &self,
        core: &Core,
        setup: EntrySetupBudget,
    ) -> Result<Moment, EntryPreparationInputError> {
        if !std::ptr::eq(self.core.as_ptr(), core) {
            return Err(EntryPreparationInputError::ForeignOwner);
        }
        if self.setup != setup {
            return Err(EntryPreparationInputError::OriginalSetupMismatch);
        }
        if lock(&self.sampled).regression {
            return Err(EntryPreparationInputError::Control(EntryPreparationControlCause::Clock(
                TimeError::Regression,
            )));
        }
        let now = core.clock.metadata().map_err(|cause| {
            self.close_publication();
            EntryPreparationInputError::Control(EntryPreparationControlCause::Unexpected(cause))
        })?;
        let state = lock(&core.state);
        let mut sampled = lock(&self.sampled);
        if now < sampled.last || now < self.setup.started() {
            sampled.regression = true;
            sampled.publication = Publication::Closed;
            return Err(EntryPreparationInputError::Control(EntryPreparationControlCause::Clock(
                TimeError::Regression,
            )));
        }
        sampled.last = now;
        if let Err(cause) = live(&state, &self.context, self.setup, now) {
            sampled.publication = Publication::Closed;
            return Err(cause);
        }
        Ok(now)
    }

    /// Register the original passive slot after full-claim-backed owner construction.
    /// A rejected slot and every final owner guard are destroyed outside Core.
    pub(crate) fn register(
        &self,
        slot: Arc<PreparationSlot>,
    ) -> Result<(), (EntryPreparationInputError, Arc<PreparationSlot>)> {
        let Some(core) = self.core.upgrade() else {
            return Err((EntryPreparationInputError::Closed, slot));
        };
        let now = match self.check(&core, self.setup) {
            Ok(now) => now,
            Err(cause) => return Err((cause, slot)),
        };
        let mut state = lock(&core.state);
        if let Err(cause) = live(&state, &self.context, self.setup, now) {
            drop(state);
            return Err((cause, slot));
        }
        let valid = state.jobs.get_mut(&self.context.job_id()).filter(|record| {
            record.phase == JobPhase::Running
                && !record.retiring
                && record.entry_setup == Some(self.setup)
                && record.preparation.is_none()
                && record.scope_id == slot.scope
                && slot.execution == core.id
                && slot.job == record.spec.id
                && record
                    .memory
                    .as_ref()
                    .is_some_and(|memory| memory.matches_lease(self.context.memory()))
        });
        match valid {
            Some(record) => {
                record.preparation = Some(slot);
                Ok(())
            }
            None => {
                drop(state);
                Err((EntryPreparationInputError::Closed, slot))
            }
        }
    }

    /// Detach only this actual slot; release its Arc/control block after leaving Core.
    /// The outer owner calls this before its final slot and original whole claim drop.
    pub(crate) fn detach(&self, slot: &Arc<PreparationSlot>) {
        if let Some(core) = self.core.upgrade() {
            let removed = {
                let mut state = lock(&core.state);
                state.jobs.get_mut(&self.context.job_id()).and_then(|record| {
                    if record.preparation.as_ref().is_some_and(|stored| Arc::ptr_eq(stored, slot)) {
                        record.preparation.take()
                    } else {
                        None
                    }
                })
            };
            drop(removed);
            core.retire_native_custody(self.context.job_id());
        }
    }
}

fn live(
    state: &State,
    context: &JobContext,
    setup: EntrySetupBudget,
    now: Moment,
) -> Result<(), EntryPreparationInputError> {
    let cutoff = context.deadline().map_or(setup.until(), |deadline| deadline.min(setup.until()));
    live_scopes(state, context.scope_id(), context.cancellation_requested(), cutoff, now)
}

/// Revalidate actual receiving-client facts after the last external provider returns.
/// This never samples or invokes ports from the supplied matching context.
impl Core {
    pub(crate) fn validate_preparation_client(
        &self,
        state: &State,
        observed: &JobContext,
        now: Moment,
        cutoff: Moment,
    ) -> Result<(), EntryPreparationInputError> {
        if observed.owner_id() != self.id {
            return Err(EntryPreparationInputError::ForeignOwner);
        }
        let record =
            state.jobs.get(&observed.job_id()).ok_or(EntryPreparationInputError::Closed)?;
        if record.scope_id != observed.scope_id()
            || record.spec != observed.spec()
            || record.deadline != observed.deadline()
            || record.memory.as_ref().is_none_or(|memory| !memory.matches_lease(observed.memory()))
        {
            return Err(EntryPreparationInputError::OriginalSetupMismatch);
        }
        if record.phase != JobPhase::Running || record.retiring || self.clock.failed() {
            return Err(EntryPreparationInputError::Closed);
        }
        let cutoff = record.deadline.map_or(cutoff, |deadline| cutoff.min(deadline));
        live_scopes(state, record.scope_id, record.job.cancellation().is_requested(), cutoff, now)
    }
}

fn live_scopes(
    state: &State,
    scope: sugarsubstitute_contracts::execution::ScopeId,
    mut cancelled: bool,
    mut cutoff: Moment,
    now: Moment,
) -> Result<(), EntryPreparationInputError> {
    let mut current = Some(scope);
    let mut ancestors = 0;
    let mut closed = state.stopping;
    while let Some(id) = current {
        // Both current scope partitions may appear in one genuine ancestry.
        if ancestors >= 64 + 132 {
            return Err(EntryPreparationInputError::Closed);
        }
        ancestors += 1;
        let scope = state.scopes.get(&id).ok_or(EntryPreparationInputError::Closed)?;
        closed |= scope.closed || scope.abandoned;
        if let Some(deadline) = scope.deadline {
            cutoff = cutoff.min(deadline);
        }
        cancelled |= scope.cancel.is_requested();
        current = scope.parent;
    }
    if now >= cutoff {
        return Err(EntryPreparationInputError::Control(EntryPreparationControlCause::Deadline));
    }
    if cancelled {
        return Err(EntryPreparationInputError::Control(EntryPreparationControlCause::Cancelled));
    }
    if closed {
        return Err(EntryPreparationInputError::Closed);
    }
    Ok(())
}
