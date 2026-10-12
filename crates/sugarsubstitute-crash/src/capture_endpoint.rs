// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve replay authority on the actual registration, across scopes and retired jobs.
use crate::capture::{Shared, lock};
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::{
    crash::CaptureBudget,
    execution::{ExecutionId, JobClass, JobContext, JobId, MemoryClaim, Operation},
    lifecycle::{CoordinatorKey, RunKey},
};
use sugarsubstitute_execution::windows_capture_attach::{
    CaptureAttachExpectation, CaptureAttachRetirementReport, CaptureEndpointObservation,
    CaptureEndpointSource, CaptureEndpointSourceError,
};

const SLOTS: usize = 64;
pub(crate) const SOURCE_BYTES: u64 = crate::input::SOURCE_STORAGE_BYTES as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Pending,
    Admitted(ExecutionId, JobId),
    NativeRetired(ExecutionId, JobId),
}
#[derive(Clone, Copy)]
struct Previous {
    instance: u64,
    generation: u64,
    budget: CaptureBudget,
    sequence: u64,
    execution: ExecutionId,
    job: JobId,
}
#[derive(Clone, Copy)]
struct Slot {
    instance: u64,
    generation: u64,
    budget: CaptureBudget,
    sequence: u64,
    phase: Phase,
    borrowed: bool,
    previous: Option<Previous>,
}

/// Fixed registration-lifetime state; no discarded job or new view can reset it.
pub(super) struct Attempts {
    owner: CoordinatorKey,
    slots: [Option<Slot>; SLOTS],
    instance_highwater: u64,
    sequence: u64,
    closing: bool,
    _memory: MemoryClaim,
}
/// One exact private reservation; Arc clones share this single bounded allocation.
pub(super) struct RegisteredSource {
    shared: Arc<Shared>,
    reservation: Reservation,
}
#[derive(Clone, Copy)]
struct Reservation {
    slot: usize,
    instance: u64,
    generation: u64,
    sequence: u64,
    budget: CaptureBudget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceFailure {
    ForeignOwner,
    Replay,
    Overloaded,
    Closing,
    Exhausted,
    ForeignAssociation,
    Storage,
}
impl fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture source association rejected: {self:?}")
    }
}
impl Error for SourceFailure {}

impl Attempts {
    pub(super) fn new(
        owner: CoordinatorKey,
        memory: MemoryClaim,
    ) -> Result<Box<Self>, SourceFailure> {
        // Arc's two counters are counted separately from every live view allocation.
        const VIEW_BYTES: usize = size_of::<RegisteredSource>() + 2 * size_of::<usize>();
        const ACTUAL_BYTES: usize = size_of::<Attempts>() + SLOTS * VIEW_BYTES;
        const _: () = assert!(ACTUAL_BYTES <= SOURCE_BYTES as usize);
        if memory.bytes() != SOURCE_BYTES || ACTUAL_BYTES > SOURCE_BYTES as usize {
            return Err(SourceFailure::Storage);
        }
        Ok(Box::new(Self {
            owner,
            slots: [None; SLOTS],
            instance_highwater: 0,
            sequence: 0,
            closing: false,
            _memory: memory,
        }))
    }
    fn reserve(
        &mut self,
        run: RunKey,
        budget: CaptureBudget,
    ) -> Result<Reservation, SourceFailure> {
        if run.owner != self.owner {
            return Err(SourceFailure::ForeignOwner);
        }
        if self.closing {
            return Err(SourceFailure::Closing);
        }
        let instance = run.instance.get();
        let generation = run.generation.get();
        let found = self.slots.iter().position(|slot| slot.is_some_and(|s| s.instance == instance));
        let (index, previous) = if let Some(index) = found {
            let old = self.slots[index].ok_or(SourceFailure::ForeignAssociation)?;
            let Phase::NativeRetired(execution, job) = old.phase else {
                return Err(SourceFailure::Replay);
            };
            if old.borrowed || generation <= old.generation {
                return Err(SourceFailure::Replay);
            }
            (
                index,
                Some(Previous {
                    instance: old.instance,
                    generation: old.generation,
                    budget: old.budget,
                    sequence: old.sequence,
                    execution,
                    job,
                }),
            )
        } else {
            let pending_highwater = self
                .slots
                .iter()
                .flatten()
                .filter(|slot| {
                    slot.phase == Phase::Pending
                        && slot.previous.is_none_or(|old| old.instance != slot.instance)
                })
                .map(|slot| slot.instance)
                .max()
                .unwrap_or(0);
            if instance <= self.instance_highwater.max(pending_highwater) {
                return Err(SourceFailure::Replay);
            }
            let index = self
                .slots
                .iter()
                .position(Option::is_none)
                .or_else(|| {
                    self.slots.iter().position(|slot| {
                        slot.is_some_and(|s| {
                            matches!(s.phase, Phase::NativeRetired(..)) && !s.borrowed
                        })
                    })
                })
                .ok_or(SourceFailure::Overloaded)?;
            let previous = self.slots[index].and_then(|old| {
                if let Phase::NativeRetired(execution, job) = old.phase {
                    Some(Previous {
                        instance: old.instance,
                        generation: old.generation,
                        budget: old.budget,
                        sequence: old.sequence,
                        execution,
                        job,
                    })
                } else {
                    None
                }
            });
            (index, previous)
        };
        let sequence = self.sequence.checked_add(1).ok_or(SourceFailure::Exhausted)?;
        self.sequence = sequence;
        self.slots[index] = Some(Slot {
            instance,
            generation,
            budget,
            sequence,
            phase: Phase::Pending,
            borrowed: true,
            previous,
        });
        Ok(Reservation { slot: index, instance, generation, sequence, budget })
    }
    fn exact(&mut self, reservation: Reservation) -> Result<&mut Slot, SourceFailure> {
        self.slots
            .get_mut(reservation.slot)
            .and_then(Option::as_mut)
            .filter(|slot| {
                slot.instance == reservation.instance
                    && slot.generation == reservation.generation
                    && slot.sequence == reservation.sequence
                    && slot.budget == reservation.budget
                    && slot.borrowed
            })
            .ok_or(SourceFailure::ForeignAssociation)
    }
    fn commit(
        &mut self,
        reservation: Reservation,
        execution: ExecutionId,
        job: JobId,
    ) -> Result<(), SourceFailure> {
        let slot = self.exact(reservation)?;
        match slot.phase {
            Phase::Pending => {
                slot.phase = Phase::Admitted(execution, job);
                slot.previous = None;
            }
            Phase::Admitted(old_execution, old_job)
            | Phase::NativeRetired(old_execution, old_job)
                if old_execution == execution && old_job == job => {}
            _ => return Err(SourceFailure::ForeignAssociation),
        }
        self.instance_highwater = self.instance_highwater.max(reservation.instance);
        Ok(())
    }
    fn retire(
        &mut self,
        reservation: Reservation,
        execution: ExecutionId,
        job: JobId,
    ) -> Result<(), SourceFailure> {
        let slot = self.exact(reservation)?;
        match slot.phase {
            Phase::Admitted(old_execution, old_job)
            | Phase::NativeRetired(old_execution, old_job)
                if old_execution == execution && old_job == job =>
            {
                slot.phase = Phase::NativeRetired(execution, job);
                Ok(())
            }
            _ => Err(SourceFailure::ForeignAssociation),
        }
    }
    fn release(&mut self, reservation: Reservation) {
        let Ok(slot) = self.exact(reservation) else { return };
        if slot.phase == Phase::Pending {
            let previous = slot.previous.map(|old| Slot {
                instance: old.instance,
                generation: old.generation,
                budget: old.budget,
                sequence: old.sequence,
                phase: Phase::NativeRetired(old.execution, old.job),
                borrowed: false,
                previous: None,
            });
            self.slots[reservation.slot] = previous;
        } else {
            slot.borrowed = false;
        }
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
    }
    pub(super) fn has_unretired(&self) -> bool {
        self.slots.iter().flatten().any(|slot| !matches!(slot.phase, Phase::NativeRetired(..)))
    }
}

impl RegisteredSource {
    pub(super) fn reserve(
        shared: &Arc<Shared>,
        run: RunKey,
        budget: CaptureBudget,
    ) -> Result<Arc<Self>, SourceFailure> {
        let reservation =
            lock(&shared.attempts).as_mut().ok_or(SourceFailure::Closing)?.reserve(run, budget)?;
        Ok(Arc::new(Self { shared: shared.clone(), reservation }))
    }
    pub(super) fn commit(&self, execution: ExecutionId, job: JobId) -> Result<(), SourceFailure> {
        lock(&self.shared.attempts).as_mut().ok_or(SourceFailure::Closing)?.commit(
            self.reservation,
            execution,
            job,
        )
    }
    fn matches(&self, expected: CaptureAttachExpectation) -> bool {
        expected.owner() == self.shared.owner
            && expected.run().instance.get() == self.reservation.instance
            && expected.run().generation.get() == self.reservation.generation
            && expected.attach_budget() == self.reservation.budget
            && expected.build_bytes() == &self.shared.build[..self.shared.build_length]
    }
}
fn source_error(cause: impl Error + Send + Sync + 'static) -> CaptureEndpointSourceError {
    CaptureEndpointSourceError::new("registered_capture_endpoint", cause)
}
#[cfg(test)]
impl RegisteredSource {
    /// Observe the exact actual source phase; this value grants no retirement authority.
    pub(crate) fn retired_association(&self) -> Option<(ExecutionId, JobId)> {
        let mut attempts = lock(&self.shared.attempts);
        let slot = attempts.as_mut()?.exact(self.reservation).ok()?;
        match slot.phase {
            Phase::NativeRetired(execution, job) => Some((execution, job)),
            Phase::Pending | Phase::Admitted(..) => None,
        }
    }
}
impl CaptureEndpointSource for RegisteredSource {
    fn observe(
        &self,
        expected: CaptureAttachExpectation,
        context: &JobContext,
    ) -> Result<CaptureEndpointObservation, CaptureEndpointSourceError> {
        if !self.matches(expected)
            || context.owner_id() != self.shared.execution_id
            || context.spec().class != JobClass::NativeObservation
            || context.spec().operation != Operation::NativeLifecycle
            || context.deadline().is_some()
        {
            return Err(source_error(SourceFailure::ForeignAssociation));
        }
        // Admitted association precedes closing/cancellation rejection, including
        // work stopped before any kernel acquisition. It performs no clock query.
        let mut attempts = lock(&self.shared.attempts);
        let owner = attempts.as_mut().ok_or_else(|| source_error(SourceFailure::Closing))?;
        owner
            .commit(self.reservation, context.owner_id(), context.job_id())
            .map_err(source_error)?;
        if owner.closing {
            return Err(source_error(SourceFailure::Closing));
        }
        let native = lock(&self.shared.native);
        let actual = native.handler.ok_or_else(|| source_error(SourceFailure::Closing))?;
        if !native.registered
            || native.retired
            || expected.handler().process_id() != actual.process_id
            || expected.handler().creation_time_100ns() != actual.creation_time_100ns
        {
            return Err(source_error(SourceFailure::ForeignAssociation));
        }
        let result = native
            .lease
            .as_ref()
            .ok_or_else(|| source_error(SourceFailure::Closing))?
            .endpoint(expected)
            .map_err(source_error);
        drop(native);
        drop(attempts);
        result
    }
    fn native_retired(
        &self,
        actual: &CaptureAttachRetirementReport,
    ) -> Result<(), CaptureEndpointSourceError> {
        if actual.run().owner != self.shared.owner
            || actual.run().instance.get() != self.reservation.instance
            || actual.run().generation.get() != self.reservation.generation
        {
            return Err(source_error(SourceFailure::ForeignAssociation));
        }
        lock(&self.shared.attempts)
            .as_mut()
            .ok_or_else(|| source_error(SourceFailure::Closing))?
            .retire(self.reservation, actual.execution_id(), actual.job_id())
            .map_err(source_error)
    }
}
impl Drop for RegisteredSource {
    fn drop(&mut self) {
        if let Some(owner) = lock(&self.shared.attempts).as_mut() {
            owner.release(self.reservation);
        }
    }
}

#[cfg(test)]
mod tests;
