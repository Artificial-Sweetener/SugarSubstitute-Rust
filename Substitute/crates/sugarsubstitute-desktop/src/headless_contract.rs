// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Application lifetime contract independent of native event-loop implementation.
use crate::StartupPreparationStep;
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::CompletionWake;

/// Request continued native pumping or begin the one owned shutdown transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadlessStep {
    /// Work remains admitted; wait for actual completion or the owner's deadline.
    Active,
    /// Close admission and begin actual owned settlement without blocking the UI.
    Shutdown,
}
/// Coordinate one application/execution lifetime on the native event-loop thread.
///
/// This owner, not the pump, decides admission, deadlines and typed job consumption.
/// All methods run on the caller's native main thread and must return promptly;
/// blocking/native joins remain with the execution adapter. A cancellation request
/// does not prove stopping. The caller retains this borrowed owner throughout errors.
pub trait HeadlessLifetime {
    /// Start once against the retained native completion wake before Tao construction.
    /// The returned wake is thread-safe and supplies no window/runtime authority.
    /// # Errors
    /// Preserve the original cause; even partial startup must support owned shutdown.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Complete this role's actual prerequisites before Tao event-loop construction.
    /// Runs on the main thread with the same wake/execution lifetime as `start`.
    /// `Ready` is control flow only; the composing owner retains actual authority.
    /// `Closing` closes admission while the pump still drains actual owned work.
    /// # Errors
    /// Preserve the original cause and remain capable of bounded owned shutdown.
    fn prepare_native_runtime(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>>;
    /// Consume at most `max` typed completion records or bounded deadline decisions.
    /// After shutdown admission closes, this only drains/acknowledges existing work.
    /// # Errors
    /// Preserve original application/execution causes while remaining shutdown-capable.
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep, Box<dyn Error>>;
    /// Return the earliest requested native wake in the same `Instant` clock epoch.
    /// A past deadline must be advanced/retired by bounded `activity`; no timer is spawned.
    fn next_deadline(&self) -> Option<Instant>;
    /// Idempotently close admission and request owned cancellation/cleanup, without joining.
    /// The pump invokes this once even if it reports an error.
    /// # Errors
    /// Preserve the original cause; subsequent polling must still observe actual settlement.
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>>;
    /// Windows is ending the session; it may end this process soon without a close. A role
    /// with a decision that must be durable before then publishes it here, on the main thread
    /// and without waiting. The session end itself arrives through the role's ordinary close.
    /// Roles with nothing to publish keep this default.
    fn session_ending(&mut self) {}
    /// Report true only after actual jobs, terminal disposal, notifications and workers settle.
    /// A deadline/cancellation/kill request cannot count as joined or native settlement.
    /// # Errors
    /// Preserve cleanup failures; remain pollable until actual settlement is observed.
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>>;
    /// Finish after proven settlement, disconnected completion wake and dropped native loop.
    /// No unresolved work is joined here on the UI thread.
    /// # Errors
    /// Preserve final owner teardown failure after the native lifetime is already gone.
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>>;
}
