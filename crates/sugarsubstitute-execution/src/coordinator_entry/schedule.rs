// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Aggregate bounded private entry deadlines into one slot of the existing timer owner.
use super::types::EntryTransportError;
use crate::native_deadlines::{Deadlines, Role};
use std::time::Duration;
use sugarsubstitute_contracts::execution::Moment;

pub(super) fn probe(now: Moment, ancestor: Moment) -> Result<Moment, EntryTransportError> {
    let remaining = ancestor.checked_duration_since(now).map_err(|cause| {
        EntryTransportError::Clock(crate::native::NativeObservationError::new(
            "entry_probe_remaining",
            cause,
        ))
    })?;
    if remaining.is_zero() {
        return Err(EntryTransportError::InvalidDeadline);
    }
    now.checked_add(remaining.min(Duration::from_secs(2))).map_err(|cause| {
        EntryTransportError::Clock(crate::native::NativeObservationError::new(
            "entry_probe_until",
            cause,
        ))
    })
}
pub(super) struct Schedule {
    latest: u64,
    armed: Option<u64>,
    deadline: Option<Moment>,
}
impl Schedule {
    pub(super) fn new() -> Self {
        Self { latest: 0, armed: Some(0), deadline: None }
    }
    pub(super) fn publish(
        &mut self,
        slots: &mut Deadlines,
        earliest: Option<Moment>,
    ) -> Result<bool, EntryTransportError> {
        if self.deadline == earliest && (self.latest != 0 || self.armed.is_none()) {
            return Ok(false);
        }
        if let Some(operation) = self.armed.take()
            && !slots.clear(Role::Handshake, operation)
        {
            return Err(EntryTransportError::InvalidPhase);
        }
        self.deadline = earliest;
        if let Some(deadline) = earliest {
            self.latest =
                self.latest.checked_add(1).ok_or(EntryTransportError::InvalidOperation)?;
            if !slots.arm(Role::Handshake, self.latest, deadline) {
                return Err(EntryTransportError::Closed);
            }
            self.armed = Some(self.latest);
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_three_slots_and_due_entry_can_be_rearmed_without_renewing_commands()
    -> Result<(), EntryTransportError> {
        let mut slots = Deadlines::handshake(Moment::from_nanos(30));
        assert!(slots.arm(Role::Read, 7, Moment::from_nanos(70)));
        assert!(slots.arm(Role::Write, 8, Moment::from_nanos(80)));
        let mut schedule = Schedule::new();
        assert!(schedule.publish(&mut slots, Some(Moment::from_nanos(20)))?);
        assert!(!schedule.publish(&mut slots, Some(Moment::from_nanos(20)))?);
        assert_eq!(slots.scan(Moment::from_nanos(20)), (Some(Moment::from_nanos(70)), true));
        schedule.publish(&mut slots, Some(Moment::from_nanos(40)))?;
        assert_eq!(slots.scan(Moment::from_nanos(30)), (Some(Moment::from_nanos(40)), false));
        assert!(schedule.publish(&mut slots, None)?);
        assert!(!schedule.publish(&mut slots, None)?);
        assert!(slots.clear(Role::Read, 7));
        assert!(slots.clear(Role::Write, 8));
        assert_eq!(slots.scan(Moment::from_nanos(100)), (None, false));
        Ok(())
    }
    #[test]
    fn final_short_probe_caps_before_addition_at_u64_max() -> Result<(), EntryTransportError> {
        let now = Moment::from_nanos(u64::MAX - 1);
        assert_eq!(probe(now, Moment::from_nanos(u64::MAX))?, Moment::from_nanos(u64::MAX));
        assert!(probe(now, now).is_err());
        assert!(probe(now, Moment::from_nanos(u64::MAX - 2)).is_err());
        Ok(())
    }
}
