// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Report startup admission limits and actual shutdown observations.

use sugarsubstitute_contracts::execution::{JobId, JobPhase, JobSpec, ScopeId};

/// Report original native output accounting retained by actual symbol and export owners.
/// Counters describe custody and never establish successful OS close or quiescence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOutputCustody {
    /// Scope that originally admitted the fixed native reservation.
    pub original_scope_id: ScopeId,
    /// Job that originally admitted the fixed native reservation.
    pub original_job_id: JobId,
    /// Unreleased output accounting guards, in native output slots.
    pub outputs_in_custody: u16,
    /// Maximum simultaneous custody observed for this original reservation.
    pub high_water: u16,
}

/// Permanent process-owned capture remains charged after temporary worker shutdown.
/// These passive facts do not claim physical PIN/module/source retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessCaptureCustody {
    /// Original genuine admitted scope.
    pub scope_id: ScopeId,
    /// Historical Registered job; its completion was acknowledged separately.
    pub job_id: JobId,
    /// Full capture target still excluded from another capture admission.
    pub full_target: sugarsubstitute_contracts::crash::CaptureTarget,
    /// Whole original reservation retained by the outside process capsule, in bytes.
    pub reservation_bytes: u64,
}

pub(crate) const DEFAULT_MEMORY_BYTES: u64 = 256 * 1024 * 1024;

/// Immutable capacity policy for one production execution owner.
///
/// Lifecycle jobs can consume idle ordinary slots while ordinary submissions
/// cannot consume lifecycle reserves. Peak bytes include caller-retained results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    /// Native observations actually registered concurrently, separate from I/O capacity.
    pub native_running: usize,
    /// Unbound or bound native reservations awaiting registration.
    pub native_queued: usize,
    /// Native held credits covering reservations, registrations, retirement and untransferred results.
    pub native_terminal: usize,
    /// Native live/retained scopes, independent of the ordinary scope allowance.
    pub native_scopes: usize,
    /// One private native registration/retirement worker on Windows.
    pub native_workers: usize,
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
    /// Ordinary/lifecycle ready factories (32), independent of native reservations.
    /// Deferred cleanup uses the corresponding class's held terminal credits.
    pub queued: usize,
    /// Maximum ordinary ready factories (28).
    pub queued_ordinary: usize,
    /// Ordinary/lifecycle held credits (48), independent of native held credits.
    /// Covers ready/running work, retained results and deferred cleanup.
    pub terminal: usize,
    /// Maximum ordinary held terminal credits.
    pub terminal_ordinary: usize,
    /// Maximum ordinary live or retained scope records, separate from native scopes.
    pub scopes: usize,
    /// Maximum peak bytes charged across jobs and retained results.
    pub memory_bytes: u64,
}

impl ExecutionLimits {
    pub(super) fn fixed(cpu_workers: usize, memory_bytes: u64) -> Self {
        Self {
            native_running: if cfg!(any(windows, test)) { 384 } else { 0 },
            native_queued: if cfg!(any(windows, test)) { 384 } else { 0 },
            native_terminal: if cfg!(any(windows, test)) { 512 } else { 0 },
            native_scopes: if cfg!(any(windows, test)) { 132 } else { 0 },
            native_workers: usize::from(cfg!(any(windows, test))),
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
            memory_bytes,
        }
    }
}

/// Observe loop return of the adapter's retained worker facilities.
///
/// A worker is finished once its loop has returned and released every value it owned;
/// its OS thread may still be exiting, and `finish_shutdown` joins it. When the last
/// worker finishes, the owner delivers exactly one completion wake, ordered after this
/// status is visible, so an observer woken by it reads every field as finished.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerStatus {
    /// The native registration/retirement worker loop has returned after quiescence.
    pub native_finished: bool,
    /// The asynchronous supervisor has returned after every retained job join completed.
    pub async_finished: bool,
    /// Every retained CPU lane loop has returned.
    pub cpu_finished: bool,
}

/// Observe an original environment allocation whose OS extent is not supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryEnvironmentCustody {
    /// Genuine original scope retained by the admitted memory lease.
    pub scope_id: sugarsubstitute_contracts::execution::ScopeId,
    /// Actual job, independently of terminal delivery acknowledgement.
    pub job_id: JobId,
    /// The OS-owned extent is unmeasured; copied units do not establish allocation size.
    pub extent: EntryEnvironmentExtent,
    /// Failed or panicked Free remains opaque, owned and nonretryable.
    pub retirement_uncertain: bool,
    /// The actual synchronous getter/read/free callback has returned.
    pub callback_quiescent: bool,
}

/// Classify actual OS allocation knowledge without inventing a handle count or byte bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryEnvironmentExtent {
    /// GetEnvironmentStringsW supplied no allocation extent.
    Unmeasured,
}

/// Observe shutdown while retaining work, untransferred results and cancellation callbacks.
#[derive(Debug)]
pub struct ShutdownProgress {
    /// Original specifications and current custody for jobs still preventing shutdown.
    pub unresolved_jobs: Vec<UnresolvedJob>,
    /// Genuine process-lifetime capture, separate from unresolved temporary native work.
    pub process_capture_custody: Vec<ProcessCaptureCustody>,
    /// Original held environment allocations; terminal ACK cannot erase these observations.
    pub entry_environment_custody: Vec<EntryEnvironmentCustody>,
    /// Nonzero original symbol output custody retained independently of job ACK.
    pub native_output_custody: Vec<NativeOutputCustody>,
    /// Every admitted job is transferred or disposed and cancellation callbacks have returned.
    /// Every retained supervisor/job join, CPU worker and native worker loop has returned.
    /// The last returning worker delivers one completion wake after its return is observable.
    /// Runtime facilities are destroyed by `finish_shutdown` after this observation.
    pub settled: bool,
    /// Effective shutdown deadline has elapsed while work remains owned.
    pub deadline_expired: bool,
    /// Jobs retaining work, unbound native permits, untransferred results,
    /// actual native retirement, deferred disposal or pending notifications.
    /// Pending scope notifications can keep shutdown unsettled even when this vector is empty.
    pub unresolved_job_ids: Vec<JobId>,
    /// Jobs awaiting a ready factory or deferred factory/result cleanup.
    pub queued: usize,
    /// Jobs executing work or supervised factory/result destruction.
    pub running: usize,
    /// Actual retained worker observations.
    pub workers: WorkerStatus,
}

/// Passive diagnostics for an admitted job; these facts grant no execution authority.
#[derive(Debug)]
pub struct UnresolvedJob {
    /// Original admitting scope.
    pub scope: ScopeId,
    /// Original operation, class, reservation and deadline.
    pub spec: JobSpec,
    /// Actual execution or terminal phase.
    pub phase: JobPhase,
    /// Whether the typed completion has been transferred or disposed.
    pub acknowledged: bool,
    /// Whether native ownership still requires checked retirement.
    pub native_held: bool,
}
