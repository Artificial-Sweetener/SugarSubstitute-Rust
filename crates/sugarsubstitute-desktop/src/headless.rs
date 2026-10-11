// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump a main-thread native lifetime without application windows or presentation.
mod state;
use crate::{
    DesktopError,
    completion_wake::{ProxyWake, WakeLifetime},
};
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::CompletionWake;
use tao::{
    event::Event, event_loop::EventLoopBuilder, platform::run_return::EventLoopExtRunReturn,
};

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
    /// Start once after the native proxy exists; early completion wakes are allowed.
    /// The returned wake is thread-safe and supplies no window/runtime authority.
    /// # Errors
    /// Preserve the original cause; even partial startup must support owned shutdown.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
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
#[derive(Clone, Copy, Debug)]
enum HeadlessAction {
    Completions,
}

/// Run one main-thread native pump with no application window, frame or focus work.
///
/// The proxy exists before `start`; each activity call receives budget 32. Active
/// waiting uses the owner's earliest deadline or actual completion. Closing uses
/// the existing bounded native cadence while keeping the loop/proxy alive until
/// actual settlement. An early native return is an error and re-enters the same
/// retained Windows event loop to continue shutdown; it never fabricates success.
/// The wake disconnects and the native loop drops before final owner teardown.
///
/// # Errors
/// Return the first original startup/activity/cleanup/native-loop failure after
/// settlement; secondary causes reach structured logging with their operation.
/// This function must run on the native main thread. It performs no process launch,
/// application-window creation, raw worker scheduling or blocking owner join.
pub fn run_headless(lifetime: &mut impl HeadlessLifetime) -> Result<(), DesktopError> {
    let mut event_loop = EventLoopBuilder::<HeadlessAction>::with_user_event().build();
    let wake_lifetime = WakeLifetime::new();
    let wake = Arc::new(ProxyWake::new(
        event_loop.create_proxy(),
        wake_lifetime.clone(),
        HeadlessAction::Completions,
    ));
    let mut state = state::HeadlessState::new();
    state.start(lifetime, wake);
    while !state.settled() {
        let exit_code = event_loop.run_return(|event, _, control| {
            match event {
                Event::UserEvent(HeadlessAction::Completions) => state.activity(lifetime),
                Event::MainEventsCleared => {
                    state.activity(lifetime);
                    state.poll_shutdown(lifetime);
                }
                _ => {}
            }
            *control = state.control(lifetime, Instant::now());
        });
        state.native_return(lifetime, exit_code);
    }
    state.finish(lifetime, || {
        wake_lifetime.disconnect();
        drop(event_loop);
    })
}
