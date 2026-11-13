// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply deterministic native outcomes without reading any process environment.

use super::super::{
    cause::EntryEnvironmentCause,
    port::{AllocationState, EnvironmentPort},
    state::NativeCapture,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Calls {
    pub(super) get: usize,
    pub(super) reads: usize,
    pub(super) free: usize,
}
#[derive(Clone, Copy, Default)]
pub(super) struct Behavior {
    pub(super) null: bool,
    pub(super) getter_panics_after_acquisition: bool,
    pub(super) read_fails_at: Option<usize>,
    pub(super) free_fails: bool,
    pub(super) free_panics: bool,
}
struct SyntheticPort {
    units: Vec<u16>,
    state: AllocationState,
    calls: Arc<Mutex<Calls>>,
    behavior: Behavior,
}
impl EnvironmentPort for SyntheticPort {
    fn get(&mut self) -> Result<(), EntryEnvironmentCause> {
        crate::scope::lock(&self.calls).get += 1;
        if self.behavior.null {
            return Err(EntryEnvironmentCause::GetNull { last_error_observation: 77 });
        }
        self.state = AllocationState::Held;
        if self.behavior.getter_panics_after_acquisition {
            std::panic::resume_unwind(Box::new("synthetic getter panic"));
        }
        Ok(())
    }
    fn read_unit(&mut self, index: usize) -> Result<u16, EntryEnvironmentCause> {
        crate::scope::lock(&self.calls).reads += 1;
        if self.behavior.read_fails_at == Some(index) {
            return Err(EntryEnvironmentCause::InvalidNativeState);
        }
        self.units.get(index).copied().ok_or(EntryEnvironmentCause::InvalidNativeState)
    }
    fn free(&mut self) -> Result<(), EntryEnvironmentCause> {
        crate::scope::lock(&self.calls).free += 1;
        self.state = AllocationState::RetirementUncertain;
        if self.behavior.free_panics {
            std::panic::resume_unwind(Box::new("synthetic free panic"));
        }
        if self.behavior.free_fails {
            return Err(EntryEnvironmentCause::InvalidNativeState);
        }
        self.state = AllocationState::Released;
        Ok(())
    }
    fn allocation_state(&self) -> AllocationState {
        self.state
    }
}
pub(super) fn capture(units: Vec<u16>, behavior: Behavior) -> (NativeCapture, Arc<Mutex<Calls>>) {
    let (port, calls) = port(units, behavior);
    (NativeCapture::new(port), calls)
}
pub(super) fn port(
    units: Vec<u16>,
    behavior: Behavior,
) -> (Box<dyn EnvironmentPort>, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let port =
        SyntheticPort { units, state: AllocationState::Vacant, calls: calls.clone(), behavior };
    (Box::new(port), calls)
}
