// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report a constant number of source-inspection boundaries using the admitted job clock.
use sugarsubstitute_contracts::execution::{JobContext, JobId, Moment, ScopeId};

/// Actual closure entry distinguishes queued work from time spent in native inspection.
pub(super) fn worker_started(context: &JobContext) {
    tracing::info!(operation="frontend_source_worker_started",job=?context.job_id(),scope=?context.scope_id(),
        original_until=?context.deadline(),observed_ns=context.now().as_nanos(),
        "Actual admitted frontend source worker began");
}

/// Fixed stages carry no selected path, filename, file contents or native capability.
#[derive(Clone, Copy, Debug)]
pub(super) enum Phase {
    SelectedObjects,
    BackendScript,
    FrontendMetadata,
    CubeMetadata,
    FrontendInventory,
    CubeInventory,
    Manifest,
}

/// Worker-local counters add no callbacks, timers, queues or retained native custody.
pub(super) struct Progress {
    job: JobId,
    scope: ScopeId,
    original_until: Option<Moment>,
    started: Moment,
    phase_started: Moment,
    phase: Phase,
    entries: u64,
    directories: u64,
    assets: u64,
    bytes: u64,
}
impl Progress {
    /// Sample only the existing injected monotonic clock after entering SystemPort.
    pub(super) fn new(context: &JobContext) -> Self {
        let now = context.now();
        Self {
            job: context.job_id(),
            scope: context.scope_id(),
            original_until: context.deadline(),
            started: now,
            phase_started: now,
            phase: Phase::SelectedObjects,
            entries: 0,
            directories: 0,
            assets: 0,
            bytes: 0,
        }
    }
    /// Count actual enumerated non-dot entries; the inspector's original bounds still reject.
    pub(super) fn entry(&mut self) {
        self.entries += 1;
    }
    /// Count actual directory enumeration attempts, without logging per directory.
    pub(super) fn directory(&mut self) {
        self.directories += 1;
    }
    /// Count only a successfully enrolled runtime asset.
    pub(super) fn asset(&mut self) {
        self.assets += 1;
    }
    /// Count bytes actually read, including script and metadata, without per-chunk events.
    pub(super) fn bytes(&mut self, count: usize) {
        self.bytes += count as u64;
    }
    /// Emit the completed previous phase before beginning the next existing operation.
    pub(super) fn phase(&mut self, next: Phase, now: Moment, handles: usize) {
        self.report(now, handles, false, true);
        self.phase = next;
        self.phase_started = now;
    }
    /// Observe success or the unchanged first error while its actual partial custody remains owned.
    pub(super) fn finish(&self, now: Moment, handles: usize, succeeded: bool) {
        self.report(now, handles, true, succeeded);
    }
    /// A regressing sample is explicit missing elapsed data, never a repaired clock or deadline.
    fn report(&self, now: Moment, handles: usize, terminal: bool, succeeded: bool) {
        let elapsed_ns = now.as_nanos().checked_sub(self.started.as_nanos());
        let phase_elapsed_ns = now.as_nanos().checked_sub(self.phase_started.as_nanos());
        tracing::info!(operation="frontend_source_inspection",job=?self.job,scope=?self.scope,
            original_until=?self.original_until,phase=?self.phase,terminal,succeeded,
            elapsed_ns,phase_elapsed_ns,entries=self.entries,directories=self.directories,
            assets=self.assets,bytes=self.bytes,retained_handles=handles,
            "Actual frontend source inspection boundary observed");
    }
}
