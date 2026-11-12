// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Delimit seven startup boundaries without allocating or sampling later frame requests.

use super::{DesktopError, TimingAudit, TimingObserver, error};

/// Select the immutable observed construction order for a native lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStartupOrder {
    /// Ordinary surfaces initialize before content execution starts.
    SurfaceFirst,
    /// Startup content authenticates and attaches capture before native surface admission.
    ExecutionFirst,
}
impl NativeStartupOrder {
    /// Return the exact seven required stages in observed order.
    pub fn stages(self) -> [NativeStartupStage; 7] {
        use NativeStartupStage::*;
        match self {
            Self::SurfaceFirst => [
                EventLoopReady,
                WindowCreated,
                PresenterReady,
                ControlsReady,
                ExecutionReady,
                FirstPreparationRequest,
                ShowFocusReturned,
            ],
            Self::ExecutionFirst => [
                ExecutionReady,
                EventLoopReady,
                WindowCreated,
                PresenterReady,
                ControlsReady,
                FirstPreparationRequest,
                ShowFocusReturned,
            ],
        }
    }
}

/// Identify an ordered native setup boundary or the first actual content request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStartupStage {
    /// Tao's event loop and its native wait infrastructure have returned from construction.
    EventLoopReady,
    /// The hidden native window and its initial monitor placement have been established.
    WindowCreated,
    /// The native presentation owner has configured its material and cursor-monitor placement.
    PresenterReady,
    /// Native accessible peers and their subclass owners have finished construction.
    ControlsReady,
    /// The content owner's start call has returned successfully with its execution lifetime.
    ExecutionReady,
    /// The owned show/focus calls have returned; this does not prove compositor exposure.
    ShowFocusReturned,
    /// The first actual content request is about to run after visibility/shape validation.
    FirstPreparationRequest,
}

/// Retain one validated raw QPC observation in the receipt's fixed startup slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStartupTiming {
    /// Exact completed setup boundary or first content-request entry.
    pub stage: NativeStartupStage,
    /// Counter value from the receipt's source, frequency and creating UI thread.
    pub ticks: i64,
}

impl TimingAudit {
    /// Reject repetition/order violations before reading the clock or committing a slot.
    pub(super) fn startup(&mut self, stage: NativeStartupStage) -> Result<(), DesktopError> {
        let index = self
            .receipt
            .startup_order
            .stages()
            .iter()
            .position(|candidate| *candidate == stage)
            .ok_or_else(|| error("startup stage is not part of the retained order"))?;
        if self.receipt.startup_milestones[index].is_some()
            || self.receipt.startup_milestones[..index].iter().any(Option::is_none)
        {
            return Err(error("startup milestone is repeated or out of order"));
        }
        let ticks = self.mark()?.ticks;
        self.receipt.startup_milestones[index] = Some(NativeStartupTiming { stage, ticks });
        Ok(())
    }

    /// Sample only the first actual visible content request, never later frame work.
    pub(super) fn first_preparation(&mut self) -> Result<(), DesktopError> {
        if self.receipt.startup_milestones[5].is_some() {
            return Ok(());
        }
        self.startup(NativeStartupStage::FirstPreparationRequest)
    }
}

/// Make disabled audits perform no counter read and retain no startup storage.
pub(crate) fn observe_startup(
    observer: Option<&TimingObserver>,
    stage: NativeStartupStage,
) -> Result<(), DesktopError> {
    observer.map(|observer| observer.startup(stage)).transpose()?;
    Ok(())
}

#[cfg(test)]
#[path = "timing_startup_tests.rs"]
mod tests;
