// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own a process-local one-millisecond timer-resolution request during visible animation.
//!
//! Windows 11 applies this request to this process, including Tao's native wait
//! thread. The guard owns no timer, worker, callback or HWND. It is UI-affine and
//! balances every successful timeBeginPeriod with the same timeEndPeriod period.
//! Fully occluded windows have no Windows guarantee of higher timer resolution.

use crate::DesktopError;
use std::{error::Error, fmt, marker::PhantomData, rc::Rc};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};

const ANIMATION_PERIOD_MS: u32 = 1;

/// Isolate the two external WinMM lifecycle calls for deterministic boundary tests.
pub(crate) trait PeriodPort {
    /// Acquire a process-local timer resolution in milliseconds.
    fn begin(&mut self, milliseconds: u32) -> Result<(), DesktopError>;
    /// Release one previously successful acquisition with its identical period.
    fn end(&mut self, milliseconds: u32) -> Result<(), DesktopError>;
}

/// Preserve WinMM's original numeric result instead of consulting unrelated last-error state.
#[derive(Debug)]
struct WinMmError {
    milliseconds: u32,
    result: u32,
}
impl fmt::Display for WinMmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "WinMM {}ms period returned code {}", self.milliseconds, self.result)
    }
}
impl Error for WinMmError {}

/// Borrow no pointers or native handles across either synchronous WinMM operation.
pub(crate) struct WinMm;
impl PeriodPort for WinMm {
    fn begin(&mut self, milliseconds: u32) -> Result<(), DesktopError> {
        // SAFETY: The period is an owned integer; WinMM retains no caller memory or callback.
        let result = unsafe { timeBeginPeriod(milliseconds) };
        native_result("begin_animation_period", milliseconds, result)
    }
    fn end(&mut self, milliseconds: u32) -> Result<(), DesktopError> {
        // SAFETY: This matches the guard's successful begin; no pointer or handle is borrowed.
        let result = unsafe { timeEndPeriod(milliseconds) };
        native_result("end_animation_period", milliseconds, result)
    }
}

/// Convert only the documented success result; keep unexpected/out-of-range codes typed.
fn native_result(
    operation: &'static str,
    milliseconds: u32,
    result: u32,
) -> Result<(), DesktopError> {
    if result == 0 {
        Ok(())
    } else {
        Err(DesktopError::new(operation, WinMmError { milliseconds, result }))
    }
}

/// Retain the successful acquisition independently of desired visibility or release failure.
pub(crate) struct AnimationPeriod<P: PeriodPort> {
    port: P,
    active: bool,
    transitions_closed: bool,
    _ui_thread: PhantomData<Rc<()>>,
}

/// Use the real Windows boundary while keeping all lifecycle policy in its owned guard.
pub(crate) type WindowsAnimationPeriod = AnimationPeriod<WinMm>;
impl WindowsAnimationPeriod {
    /// Begin inactive; constructing a hidden/static native surface changes no timer resolution.
    pub(crate) fn new() -> Self {
        Self::with_port(WinMm)
    }
}

impl<P: PeriodPort> AnimationPeriod<P> {
    /// Bind one boundary to a non-Send UI lifetime without requesting a resolution yet.
    fn with_port(port: P) -> Self {
        Self { port, active: false, transitions_closed: false, _ui_thread: PhantomData }
    }

    /// Acquire only for visible animated running content, releasing on every other mode.
    ///
    /// An unchanged mode performs no native call. The first transition error freezes
    /// automatic transitions; its caller must enter closing and run explicit shutdown.
    /// Failed begin owns no request. Failed end retains the actual acquisition for retry.
    pub(crate) fn reconcile(&mut self, required: bool) -> Result<(), DesktopError> {
        if self.transitions_closed || required == self.active {
            return Ok(());
        }
        let result = if required {
            self.port.begin(ANIMATION_PERIOD_MS)
        } else {
            self.port.end(ANIMATION_PERIOD_MS)
        };
        match result {
            Ok(()) => {
                self.active = required;
                Ok(())
            }
            Err(cause) => {
                self.transitions_closed = true;
                Err(cause)
            }
        }
    }

    /// Retry a retained matching release before native teardown, including failed transitions.
    ///
    /// This permanently closes acquisition. Failure keeps ownership for an explicit
    /// release retry or the final Drop fallback.
    pub(crate) fn shutdown(&mut self) -> Result<(), DesktopError> {
        self.transitions_closed = true;
        if self.active {
            self.port.end(ANIMATION_PERIOD_MS)?;
            self.active = false;
        }
        Ok(())
    }
}

impl<P: PeriodPort> Drop for AnimationPeriod<P> {
    /// Balance abandoned early-return paths and report any final native cleanup failure.
    fn drop(&mut self) {
        if let Err(cause) = self.shutdown() {
            tracing::error!(operation = "drop_animation_period", cause = %cause,
                "Owned animation timer resolution could not be released");
        }
    }
}

#[cfg(test)]
#[path = "windows_animation_period_tests.rs"]
mod tests;
