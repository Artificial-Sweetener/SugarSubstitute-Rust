// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Measure Windows physical memory, commit headroom, the low-memory signal and process footprints.
//!
//! Every query is nonblocking and returns in microseconds, so the desktop thread may sample
//! directly at the budget's cadence without delegating to a worker.

use super::{OsPressure, SystemMemory, SystemMemorySample};
use std::io;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        Memory::{
            CreateMemoryResourceNotification, LowMemoryResourceNotification,
            QueryMemoryResourceNotification,
        },
        ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
        },
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

/// Own the system low-memory notification handle used to sample pressure.
///
/// Construct and use it on one thread; the handle closes when the sampler drops.
pub struct SystemMemorySampler {
    low: HANDLE,
}

impl SystemMemorySampler {
    /// Open the system's low-memory resource notification.
    ///
    /// # Errors
    /// Preserves the Windows error when the notification object cannot be opened.
    pub fn new() -> io::Result<Self> {
        // SAFETY: the call takes a plain enum and returns an owned handle closed in Drop.
        let low = unsafe { CreateMemoryResourceNotification(LowMemoryResourceNotification) }
            .map_err(io::Error::other)?;
        Ok(Self { low })
    }
}

impl SystemMemory for SystemMemorySampler {
    /// Processes that exited or deny access contribute nothing; they no longer hold memory
    /// the application could reclaim.
    fn sample(&self, processes: &[u32]) -> io::Result<SystemMemorySample> {
        let mut status =
            MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        // SAFETY: `status` is a correctly sized, writable MEMORYSTATUSEX with dwLength set.
        unsafe { GlobalMemoryStatusEx(&raw mut status) }.map_err(io::Error::other)?;
        let mut low = windows::core::BOOL::default();
        // SAFETY: `self.low` is the live notification handle and `low` is a writable BOOL.
        unsafe { QueryMemoryResourceNotification(self.low, &raw mut low) }
            .map_err(io::Error::other)?;
        // SAFETY: the pseudo-handle for the current process needs no closing.
        let mut footprint = private_bytes(unsafe { GetCurrentProcess() }).unwrap_or(0);
        for &process in processes {
            footprint = footprint.saturating_add(process_private_bytes(process));
        }
        Ok(SystemMemorySample {
            total_bytes: status.ullTotalPhys,
            available_bytes: status.ullAvailPhys,
            commit_available_bytes: Some(status.ullAvailPageFile),
            os_pressure: if low.as_bool() { OsPressure::High } else { OsPressure::None },
            footprint_bytes: footprint,
        })
    }
}

impl Drop for SystemMemorySampler {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `new` and is closed exactly once here.
        if let Err(error) = unsafe { CloseHandle(self.low) } {
            tracing::warn!(operation = "memory_budget.sampler_close", %error);
        }
    }
}

fn process_private_bytes(process: u32) -> u64 {
    // SAFETY: OpenProcess returns an owned handle that is closed below on every path.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process) })
    else {
        return 0;
    };
    let bytes = private_bytes(handle).unwrap_or(0);
    // SAFETY: `handle` came from the successful OpenProcess above and is closed once.
    if let Err(error) = unsafe { CloseHandle(handle) } {
        tracing::warn!(operation = "memory_budget.process_close", process, %error);
    }
    bytes
}

fn private_bytes(process: HANDLE) -> windows::core::Result<u64> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    // SAFETY: the EX layout begins with PROCESS_MEMORY_COUNTERS and `cb` states its full size.
    unsafe {
        GetProcessMemoryInfo(
            process,
            (&raw mut counters).cast::<PROCESS_MEMORY_COUNTERS>(),
            counters.cb,
        )
    }?;
    Ok(counters.PrivateUsage as u64)
}
