// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded application scheduling, cancellation, result delivery and worker teardown.

mod admission;
mod blocking;
mod clock;
mod cpu;
#[cfg(any(windows, test))]
mod fixture_session;
mod mailbox;
mod memory;
#[cfg(any(windows, test))]
mod native;
mod runtime;
mod scope;
mod task;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub mod windows_fixture;

pub use clock::MonotonicClock;
pub use runtime::{
    ExecutionClient, ExecutionLimits, ExecutionOwner, ShutdownProgress, WorkerStatus,
};

#[cfg(test)]
mod runtime_test_support;
