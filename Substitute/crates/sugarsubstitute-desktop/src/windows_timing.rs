// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the creating UI thread's raw Windows performance counter without owning handles.
//!
//! Each FFI call borrows a valid stack output or queries the current calling thread.
//! No callback, native handle, scheduling primitive or pointer survives a call.

use crate::{
    DesktopError,
    timing::{QPC_SOURCE, Stamp, TimingClock},
};
use windows::Win32::System::{
    Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
    Threading::GetCurrentThreadId,
};

/// Bind frequency and UI identity to the thread constructing this adapter.
pub(crate) struct WindowsClock {
    frequency: i64,
    thread: u32,
}
impl WindowsClock {
    /// Query fixed QPC frequency before allocating or entering native callbacks.
    pub(crate) fn new() -> Result<Self, DesktopError> {
        let mut frequency = 0;
        // SAFETY: Windows writes one i64 to this live stack slot; no pointer is retained.
        unsafe { QueryPerformanceFrequency(&raw mut frequency) }
            .map_err(|cause| DesktopError::new("query_performance_frequency", cause))?;
        // SAFETY: This returns the current native thread identity and borrows no handles.
        let thread = unsafe { GetCurrentThreadId() };
        Ok(Self { frequency, thread })
    }
}
impl TimingClock for WindowsClock {
    fn frequency(&self) -> i64 {
        self.frequency
    }
    fn thread(&self) -> u32 {
        self.thread
    }
    fn read(&mut self) -> Result<Stamp, DesktopError> {
        let mut ticks = 0;
        // SAFETY: Windows writes one i64 to a valid stack slot during this synchronous query.
        unsafe { QueryPerformanceCounter(&raw mut ticks) }
            .map_err(|cause| DesktopError::new("query_performance_counter", cause))?;
        // SAFETY: This observes the calling thread, enabling the collector's affinity check.
        let thread = unsafe { GetCurrentThreadId() };
        Ok(Stamp { ticks, thread, source: QPC_SOURCE })
    }
}
