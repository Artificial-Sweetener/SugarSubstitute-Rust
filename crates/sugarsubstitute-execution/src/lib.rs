// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded application scheduling, cancellation, result delivery and worker teardown.

mod admission;
mod blocking;
mod clock;
mod cpu;
mod mailbox;
mod memory;
mod runtime;
mod scope;
mod task;

pub use clock::MonotonicClock;
pub use runtime::{
    ExecutionClient, ExecutionLimits, ExecutionOwner, ShutdownProgress, WorkerStatus,
};

#[cfg(test)]
mod runtime_test_support;
