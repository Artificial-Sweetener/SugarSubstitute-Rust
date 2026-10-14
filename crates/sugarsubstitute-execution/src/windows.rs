// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe owned Windows processes through admitted, cancellable execution lifetimes.
//!
//! Reservation precedes process creation. Binding consumes an owned input handle;
//! validation, non-inheritable duplication, waiting and retirement stay private.
//! Cancellation stops observation after callback quiescence and never terminates
//! the process. Unregistering waits can block on the owned retirement worker;
//! an expired shutdown deadline never proves native work has stopped.

use crate::ExecutionClient;
pub use crate::native::{
    NativeCorrelation, NativeObservationError, ProcessExit, ProcessWatchPermit,
};
use std::os::windows::io::OwnedHandle;
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, OwnedJob, Scope, ScopeSpec};

impl ExecutionClient {
    /// Admit one of 132 native scopes without consuming the 64 ordinary scope allowance.
    ///
    /// Parent cancellation and the earliest ancestor deadline remain effective.
    /// Retained results/guards and children keep their scope charged until settled.
    /// Native scopes accept typed native reservations; task factories require
    /// ordinary scopes. An ordinary or native parent may own either scope kind.
    pub fn create_native_scope(
        &self,
        spec: ScopeSpec,
        parent: Option<&Scope>,
    ) -> Result<Scope, AdmissionError> {
        self.core.create_scope_kind(spec, parent, true)
    }
    /// Reserve one native-ready slot, terminal credit and peak memory before binding.
    ///
    /// Requires NativeLifecycle/NativeObservation, a native scope and at least 4096
    /// reserved bytes. All identity/class/deadline/capacity rejections occur here.
    pub fn reserve_process_watch(
        &self,
        scope: &Scope,
        spec: JobSpec,
        correlation: NativeCorrelation,
    ) -> Result<ProcessWatchPermit, AdmissionError> {
        crate::native::reserve(&self.core, scope, spec, correlation)
    }
}
impl ProcessWatchPermit {
    /// Retire an unused reservation while preserving its observed terminal delivery.
    ///
    /// Consumes the unbound permit, preventing later binding, and requests cancellation.
    /// Execution publishes the typed outcome after actual reservation retirement;
    /// an already expired deadline retains its timeout classification. Taking that
    /// result acknowledges delivery, while its memory guard retains the byte charge.
    /// No process handle is accepted and no native process is created or terminated.
    pub fn cancel_unbound(self) -> OwnedJob<ProcessExit, NativeObservationError> {
        self.cancel_input()
    }

    /// Consume a process handle without a second capacity decision or native UI wait.
    ///
    /// The execution worker duplicates it non-inheritably and validates kernel PID
    /// and creation time. Wrong object types or insufficient rights produce an owned
    /// typed failure. The caller's authoritative process remains running on cancel.
    pub fn bind(self, process: OwnedHandle) -> OwnedJob<ProcessExit, NativeObservationError> {
        self.bind_input(Box::new(process::Lease::new(process)))
    }
}

#[allow(unsafe_code)]
#[path = "windows_process.rs"]
pub(crate) mod process;

#[cfg(test)]
#[allow(unsafe_code)]
#[path = "windows_process_tests.rs"]
mod tests;
