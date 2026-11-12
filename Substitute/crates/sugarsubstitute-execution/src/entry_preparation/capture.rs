// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy original units through finite controls and one checked native retirement.

use super::{
    cause::EntryEnvironmentCause,
    control::EntryPreparationControlCause,
    memory::copy_storage,
    port::{AllocationState, EnvironmentPort},
    state::{CaptureDisposition, EntryEnvironmentPhase, NativeCapture},
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use sugarsubstitute_contracts::coordinator_entry::MAX_ENTRY_ENVIRONMENT_UNITS;

impl NativeCapture {
    pub(crate) fn capture(
        &mut self,
        control: &mut impl FnMut() -> Result<(), EntryPreparationControlCause>,
    ) -> CaptureDisposition {
        if self.phase != EntryEnvironmentPhase::Prepared {
            self.record(EntryEnvironmentCause::InvalidNativeState);
            return self.disposition();
        }
        if !self.check(control) {
            self.phase = EntryEnvironmentPhase::Retiring;
            return self.disposition();
        }
        self.phase = EntryEnvironmentPhase::GetterInFlight;
        self.native_settled = false;
        let acquired = self.call(|port| port.get());
        self.adopt(acquired);
        self.check(control);
        if self.first.is_none() && self.port.allocation_state() != AllocationState::Held {
            self.record(EntryEnvironmentCause::InvalidNativeState);
        }
        if self.first.is_none() {
            self.phase = EntryEnvironmentPhase::Held;
            self.copy(control);
        }
        self.retire(control);
        self.disposition()
    }

    fn copy(&mut self, control: &mut impl FnMut() -> Result<(), EntryPreparationControlCause>) {
        self.phase = EntryEnvironmentPhase::CopyInFlight;
        let mut previous_zero = false;
        let mut terminated = false;
        for index in 0..MAX_ENTRY_ENVIRONMENT_UNITS {
            if index % 256 == 0 && !self.check(control) {
                return;
            }
            let read = self.call(|port| port.read_unit(index));
            let Some(unit) = self.adopt(read) else {
                self.check(control);
                return;
            };
            self.inspected = index + 1;
            terminated = previous_zero && unit == 0;
            previous_zero = unit == 0;
            if terminated {
                break;
            }
        }
        if !terminated {
            self.record(EntryEnvironmentCause::TooLarge { inspected_units: self.inspected });
        }
        if !self.check(control) || self.first.is_some() {
            return;
        }
        let storage = copy_storage(self.inspected);
        let Some(storage) = self.adopt(storage) else {
            self.check(control);
            return;
        };
        self.units = Some(storage);
        for index in 0..self.inspected {
            if index % 256 == 0 && !self.check(control) {
                return;
            }
            let read = self.call(|port| port.read_unit(index));
            let Some(unit) = self.adopt(read) else {
                self.check(control);
                return;
            };
            if let Some(units) = &mut self.units {
                units[index] = unit;
            }
        }
        if self.check(control) && self.first.is_none() {
            self.phase = EntryEnvironmentPhase::CopyChecked;
        }
    }

    /// Permit only the first Free of the identical already acquired block, after
    /// synchronous getter/read/factory access has actually returned to this owner.
    /// Original cancellation or expiry blocks new Get/read/publication but does
    /// not prohibit that owned cleanup. Pre/post controls are caught and retained
    /// without replacing the first cause or creating a new deadline/job/budget.
    /// Failed or panicked Free remains unsettled and is never attempted again.
    pub(crate) fn retire(
        &mut self,
        control: &mut impl FnMut() -> Result<(), EntryPreparationControlCause>,
    ) {
        if matches!(
            self.phase,
            EntryEnvironmentPhase::RetirementUncertain
                | EntryEnvironmentPhase::Released
                | EntryEnvironmentPhase::Delivered
        ) {
            return;
        }
        match self.port.allocation_state() {
            AllocationState::Vacant => {
                self.phase = EntryEnvironmentPhase::Retiring;
                self.native_settled = true;
                return;
            }
            AllocationState::RetirementUncertain => {
                self.phase = EntryEnvironmentPhase::RetirementUncertain;
                return;
            }
            AllocationState::Released => {
                // No checked owner Free outcome was adopted in this path.
                self.record(EntryEnvironmentCause::InvalidNativeState);
                self.phase = EntryEnvironmentPhase::RetirementUncertain;
                return;
            }
            AllocationState::Held => {}
        }
        self.check(control);
        self.phase = EntryEnvironmentPhase::FreeInFlight;
        let released = self.call(|port| port.free());
        let succeeded = self.adopt(released).is_some();
        self.phase = if succeeded && self.port.allocation_state() == AllocationState::Released {
            self.native_settled = true;
            EntryEnvironmentPhase::Released
        } else {
            if succeeded {
                self.record(EntryEnvironmentCause::InvalidNativeState);
            }
            EntryEnvironmentPhase::RetirementUncertain
        };
        self.check(control);
    }

    fn call<T>(
        &mut self,
        action: impl FnOnce(&mut dyn EnvironmentPort) -> Result<T, EntryEnvironmentCause>,
    ) -> Result<T, EntryEnvironmentCause> {
        match catch_unwind(AssertUnwindSafe(|| action(self.port.as_mut()))) {
            Ok(outcome) => outcome,
            Err(payload) => Err(EntryEnvironmentCause::ProviderPanic {
                source: crate::task::panic_cause(payload),
            }),
        }
    }
    fn adopt<T>(&mut self, outcome: Result<T, EntryEnvironmentCause>) -> Option<T> {
        match outcome {
            Ok(value) => Some(value),
            Err(cause) => {
                self.record(cause);
                None
            }
        }
    }
    fn check(
        &mut self,
        control: &mut impl FnMut() -> Result<(), EntryPreparationControlCause>,
    ) -> bool {
        let outcome = match catch_unwind(AssertUnwindSafe(control)) {
            Ok(outcome) => outcome,
            Err(payload) => {
                Err(EntryPreparationControlCause::ProviderPanic(crate::task::panic_cause(payload)))
            }
        };
        match outcome {
            Ok(()) => true,
            Err(source) => {
                self.record(EntryEnvironmentCause::Control { source });
                false
            }
        }
    }
}
