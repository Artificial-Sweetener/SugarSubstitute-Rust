// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded application scheduling, cancellation, result delivery and worker teardown.

mod admission;
mod blocking;
mod clock;
mod clock_retirement;
#[cfg(any(windows, test))]
mod counter_deadline;
mod cpu;
#[cfg(any(windows, test))]
mod fixture_forwarding;
#[cfg(any(windows, test))]
mod fixture_session;
#[cfg(any(windows, test))]
mod lifecycle_pipe;
mod mailbox;
mod memory;
#[cfg(any(windows, test))]
mod native;
#[cfg(any(windows, test))]
mod native_deadlines;
#[cfg(any(windows, test))]
#[allow(unsafe_code)]
mod native_pipe;
#[cfg(any(windows, test))]
mod output;
mod runtime;
mod scope;
mod task;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_fixture_forwarding;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub mod windows_fixture;
#[cfg(windows)]
pub mod windows_lifecycle_pipe;
#[cfg(windows)]
pub mod windows_output;

pub use clock::MonotonicClock;
pub use runtime::{
    ExecutionClient, ExecutionLimits, ExecutionOwner, ShutdownProgress, WorkerStatus,
};

#[cfg(test)]
mod runtime_test_support;

#[cfg(any(windows, test))]
mod coordinator_entry;
#[cfg(any(windows, test))]
pub use coordinator_entry::*;
#[cfg(windows)]
mod windows_coordinator_entry;

#[cfg(windows)]
mod windows_clock;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_counter;
