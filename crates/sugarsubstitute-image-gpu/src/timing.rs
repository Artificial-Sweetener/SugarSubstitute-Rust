// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fixed-size, nonblocking observations of native GPU calls; wall time is not GPU execution time.
use std::{
    sync::{Arc, Mutex, TryLockError},
    time::{Duration, Instant},
};

use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

const PHASES: usize = 10;
/// Native operation categories share one fixed inventory per selected device.
#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Encode,
    Upload,
    Submit,
    Wait,
    MapRequest,
    MapWait,
    Acquire,
    Present,
    Batch,
    Coordination,
}
impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Encode => "command_finish",
            Self::Upload => "upload",
            Self::Submit => "queue_submit",
            Self::Wait => "submission_wait",
            Self::MapRequest => "map_request",
            Self::MapWait => "mapping_wait",
            Self::Acquire => "surface_acquire",
            Self::Present => "queue_present",
            Self::Batch => "dag_batch",
            Self::Coordination => "submission_admission",
        }
    }
    fn all() -> [Self; PHASES] {
        [
            Self::Encode,
            Self::Upload,
            Self::Submit,
            Self::Wait,
            Self::MapRequest,
            Self::MapWait,
            Self::Acquire,
            Self::Present,
            Self::Batch,
            Self::Coordination,
        ]
    }
}
/// Completed work and current outstanding calls; busy_since describes continuous occupancy.
#[derive(Clone, Copy, Default)]
struct Sample {
    count: u64,
    total_us: u64,
    maximum_us: u64,
    bytes: u64,
    items: u64,
    timeouts: u64,
    active: u64,
    peak_active: u64,
    busy_since: Duration,
}
impl Sample {
    fn begin(&mut self, now: Duration) {
        if self.active == 0 {
            self.busy_since = now;
        }
        self.active = self.active.saturating_add(1);
        self.peak_active = self.peak_active.max(self.active);
    }
    fn finish(&mut self, elapsed: Duration, bytes: u64, items: u64, timeouts: u64) {
        let micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        self.count = self.count.saturating_add(1);
        self.total_us = self.total_us.saturating_add(micros);
        self.maximum_us = self.maximum_us.max(micros);
        self.bytes = self.bytes.saturating_add(bytes);
        self.items = self.items.saturating_add(items);
        self.timeouts = self.timeouts.saturating_add(timeouts);
        self.active = self.active.saturating_sub(1);
    }
}
#[derive(Default)]
struct Window {
    samples: [Sample; PHASES],
    last_report: Duration,
}
impl Window {
    fn take(&mut self, now: Duration, force: bool) -> Option<([Sample; PHASES], Duration)> {
        let elapsed = now.saturating_sub(self.last_report);
        if !force && elapsed < Duration::from_secs(1) {
            return None;
        }
        let samples = self.samples;
        self.samples = samples.map(|sample| Sample {
            active: sample.active,
            peak_active: sample.active,
            busy_since: sample.busy_since,
            ..Sample::default()
        });
        self.last_report = now;
        Some((samples, elapsed))
    }
}
struct State {
    _memory: MemoryClaim,
    origin: Instant,
    vendor: u32,
    device: u32,
    window: Mutex<Window>,
}
/// Diagnostic-only device observations. Clones retain bounded metadata admission without a native
/// device, window or execution job. Reporting never performs GPU operations or waits for its lock.
#[derive(Clone)]
pub struct GpuDiagnostics(Arc<State>);
impl GpuDiagnostics {
    /// Retain admission until the final observation handle and native span retire.
    /// # Errors
    /// Preserves the original diagnostic-metadata admission failure.
    pub(crate) fn new(
        vendor: u32,
        device: u32,
        memory: &MemoryLease,
    ) -> Result<Self, crate::GpuError> {
        let claim = memory
            .claim(size_of::<State>() as u64 + 64)
            .map_err(|cause| crate::GpuError::new("gpu_diagnostic_memory", cause))?;
        Ok(Self(Arc::new(State {
            _memory: claim,
            origin: Instant::now(),
            vendor,
            device,
            window: Mutex::new(Window::default()),
        })))
    }
    /// Emit at most one bounded phase inventory per active second. Safe on the native input
    /// thread: contention skips this observation and retains all samples for a later report.
    pub fn report(&self) {
        self.publish(false);
    }
    fn publish(&self, force: bool) {
        let now = self.0.origin.elapsed();
        let snapshot = {
            let mut window = match self.0.window.try_lock() {
                Ok(window) => window,
                Err(TryLockError::Poisoned(cause)) => cause.into_inner(),
                Err(TryLockError::WouldBlock) => return,
            };
            window.take(now, force)
        };
        if let Some((samples, elapsed)) = snapshot {
            for (phase, sample) in Phase::all().into_iter().zip(samples) {
                if sample.count == 0 && sample.peak_active == 0 {
                    continue;
                }
                self.0.emit(phase, sample, elapsed, now);
            }
        }
    }
    /// Record outstanding custody before entering native code; no lock spans a driver call.
    pub(crate) fn begin(&self, phase: Phase, bytes: u64, items: u64) -> Span {
        let now = self.0.origin.elapsed();
        self.0.window.lock().unwrap_or_else(std::sync::PoisonError::into_inner).samples
            [phase as usize]
            .begin(now);
        Span { diagnostics: self.clone(), phase, started: now, bytes, items, timeouts: 0 }
    }
}
/// A phase stays observable during a blocked driver call and records completion on every return.
pub(crate) struct Span {
    diagnostics: GpuDiagnostics,
    phase: Phase,
    started: Duration,
    bytes: u64,
    items: u64,
    timeouts: u64,
}
impl Span {
    /// Count actual native polling timeouts independently of successful observations.
    pub(crate) fn timeout(&mut self) {
        self.timeouts = self.timeouts.saturating_add(1);
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let now = self.diagnostics.0.origin.elapsed();
        self.diagnostics.0.window.lock().unwrap_or_else(std::sync::PoisonError::into_inner).samples
            [self.phase as usize]
            .finish(now.saturating_sub(self.started), self.bytes, self.items, self.timeouts);
        self.diagnostics.report();
    }
}
impl State {
    /// Emit only after releasing bookkeeping; fields are bounded numeric quantities and fixed names.
    fn emit(&self, phase: Phase, sample: Sample, elapsed: Duration, now: Duration) {
        tracing::info!(
            operation = "image_gpu_timing",
            vendor = self.vendor,
            device = self.device,
            phase = phase.name(),
            window_microseconds = elapsed.as_micros(),
            completed = sample.count,
            total_microseconds = sample.total_us,
            max_microseconds = sample.maximum_us,
            bytes = sample.bytes,
            work_items = sample.items,
            wait_timeouts = sample.timeouts,
            active = sample.active,
            peak_active = sample.peak_active,
            busy_microseconds = if sample.active == 0 {
                0
            } else {
                now.saturating_sub(sample.busy_since).as_micros()
            },
            "Native GPU wall-time and workload window"
        );
    }
}
impl Drop for State {
    fn drop(&mut self) {
        // The last handle has no outstanding spans; retain the final partial diagnostic window.
        let now = self.origin.elapsed();
        let window = self.window.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        let snapshot = window.take(now, true);
        if let Some((samples, elapsed)) = snapshot {
            for (phase, sample) in Phase::all().into_iter().zip(samples) {
                if sample.count > 0 {
                    self.emit(phase, sample, elapsed, now);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
