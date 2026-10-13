// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original process block across exclusive bounded reads and one checked Free.
#![allow(unsafe_code)]

use crate::entry_preparation::{
    cause::EntryEnvironmentCause,
    port::{AllocationState, EnvironmentPort},
};
use sugarsubstitute_contracts::coordinator_entry::MAX_ENTRY_ENVIRONMENT_UNITS;
use windows::{
    Win32::{
        Foundation::GetLastError,
        System::Environment::{FreeEnvironmentStringsW, GetEnvironmentStringsW},
    },
    core::PWSTR,
};

/// Keep one original read-only block; destruction is not proof of native release.
/// The outer preparation owner retains this provider until checked Free or
/// unresolved process teardown. This adapter never retries an uncertain Free.
pub(crate) struct WindowsEnvironment {
    pointer: Option<PWSTR>,
    getter_called: bool,
    free_attempted: bool,
    inspected: usize,
    previous_zero: bool,
    terminated: bool,
    released: bool,
}

// SAFETY: this nonclone owner transfers the unique process allocation only while
// no synchronous call can access it. All reads and Unicode Free require &mut self;
// outer custody keeps callbacks outside concurrent retirement. Neither the raw
// pointer nor borrowed memory is exposed, and no thread-local state is retained.
unsafe impl Send for WindowsEnvironment {}

impl WindowsEnvironment {
    /// Prepare stable ownership without reading or allocating an OS environment block.
    pub(crate) const fn new() -> Self {
        Self {
            pointer: None,
            getter_called: false,
            free_attempted: false,
            inspected: 0,
            previous_zero: false,
            terminated: false,
            released: false,
        }
    }
}
impl EnvironmentPort for WindowsEnvironment {
    fn get(&mut self) -> Result<(), EntryEnvironmentCause> {
        if self.getter_called {
            return Err(EntryEnvironmentCause::InvalidNativeState);
        }
        self.getter_called = true;
        // SAFETY: Get takes no arguments. Immediately retain its original pointer
        // inside this pre-existing owner before allocation or arbitrary callbacks.
        let pointer = unsafe { GetEnvironmentStringsW() };
        if pointer.is_null() {
            // Get's documentation promises NULL failure, not GetLastError detail.
            // SAFETY: observing thread-local raw last-error requires no input.
            let observed = unsafe { GetLastError() }.0;
            return Err(EntryEnvironmentCause::GetNull { last_error_observation: observed });
        }
        self.pointer = Some(pointer);
        Ok(())
    }

    fn read_unit(&mut self, index: usize) -> Result<u16, EntryEnvironmentCause> {
        if self.free_attempted || index > self.inspected {
            return Err(EntryEnvironmentCause::InvalidNativeState);
        }
        if index >= MAX_ENTRY_ENVIRONMENT_UNITS {
            return Err(EntryEnvironmentCause::TooLarge { inspected_units: self.inspected });
        }
        if self.terminated && index >= self.inspected {
            return Err(EntryEnvironmentCause::InvalidNativeState);
        }
        let pointer = self.pointer.ok_or(EntryEnvironmentCause::InvalidNativeState)?;
        // SAFETY: Get supplies a read-only double-NUL-terminated OS block. New
        // reads advance by one unit only, stopping at its first double NUL and
        // the policy cap. Re-reads stay inside that already inspected prefix.
        // The unique owner has not attempted Free; no borrowed slice escapes.
        let unit = unsafe { pointer.0.add(index).read() };
        if index == self.inspected {
            self.inspected += 1;
            self.terminated = self.previous_zero && unit == 0;
            self.previous_zero = unit == 0;
        }
        Ok(unit)
    }

    fn free(&mut self) -> Result<(), EntryEnvironmentCause> {
        if self.free_attempted {
            return Err(EntryEnvironmentCause::InvalidNativeState);
        }
        let pointer = self.pointer.ok_or(EntryEnvironmentCause::InvalidNativeState)?;
        self.free_attempted = true;
        // SAFETY: pass the exact non-null original Unicode Get pointer once.
        // &mut self excludes active readers. Checked failure keeps this pointer
        // opaque; the outer owner cannot interpret failure as safe retry or release.
        unsafe { FreeEnvironmentStringsW(pointer) }
            .map_err(|source| EntryEnvironmentCause::Free { source })?;
        self.pointer = None;
        self.released = true;
        Ok(())
    }

    fn allocation_state(&self) -> AllocationState {
        if self.released {
            AllocationState::Released
        } else if self.free_attempted {
            AllocationState::RetirementUncertain
        } else if self.pointer.is_some() {
            AllocationState::Held
        } else {
            AllocationState::Vacant
        }
    }
}
