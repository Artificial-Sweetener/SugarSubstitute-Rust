// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Report fixed admission limits and actual shutdown observations.

use sugarsubstitute_contracts::execution::JobId;

/// Fixed capacity policy for the production execution owner.
///
/// Lifecycle jobs can consume idle ordinary slots while ordinary submissions
/// cannot consume lifecycle reserves. Peak bytes include caller-retained results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    /// Private asynchronous runtime worker count.
    pub async_workers: usize,
    /// Maximum finite blocking workers.
    pub blocking_workers: usize,
    /// Fixed CPU workers; one on a single-logical-CPU machine, otherwise two.
    pub cpu_workers: usize,
    /// Total concurrent asynchronous I/O work.
    pub io_running: usize,
    /// Maximum ordinary I/O work.
    pub io_ordinary: usize,
    /// Total ready factories awaiting execution (32); deferred cleanup uses terminal credits.
    pub queued: usize,
    /// Maximum ordinary ready factories (28).
    pub queued_ordinary: usize,
    /// Total held credits (48), covering ready/running work, retained results and deferred cleanup.
    pub terminal: usize,
    /// Maximum ordinary held terminal credits.
    pub terminal_ordinary: usize,
    /// Maximum live or retained scope records.
    pub scopes: usize,
    /// Maximum peak bytes charged across jobs and retained results.
    pub memory_bytes: u64,
}

impl ExecutionLimits {
    pub(super) fn fixed(cpu_workers: usize) -> Self {
        Self {
            async_workers: 2,
            blocking_workers: 2,
            cpu_workers,
            io_running: 12,
            io_ordinary: 8,
            queued: 32,
            queued_ordinary: 28,
            terminal: 48,
            terminal_ordinary: 44,
            scopes: 64,
            memory_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Observe actual termination of the adapter's retained worker facilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerStatus {
    /// Every asynchronous supervisor/job join has actually completed.
    pub async_finished: bool,
    /// Every retained CPU thread has actually returned.
    pub cpu_finished: bool,
}

/// Observe shutdown while retaining work, untransferred results and cancellation callbacks.
#[derive(Debug)]
pub struct ShutdownProgress {
    /// Every admitted job is transferred or disposed and cancellation callbacks have returned.
    /// Every retained supervisor/job join and CPU worker has actually stopped.
    /// Runtime facilities are destroyed by `finish_shutdown` after this observation.
    pub settled: bool,
    /// Effective shutdown deadline has elapsed while work remains owned.
    pub deadline_expired: bool,
    /// Jobs retaining work, untransferred results, deferred disposal or pending notifications.
    /// Pending scope notifications can keep shutdown unsettled even when this vector is empty.
    pub unresolved_job_ids: Vec<JobId>,
    /// Jobs awaiting a ready factory or deferred factory/result cleanup.
    pub queued: usize,
    /// Jobs executing work or supervised factory/result destruction.
    pub running: usize,
    /// Actual retained worker observations.
    pub workers: WorkerStatus,
}
