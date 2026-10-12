// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain pre-Tao completion pumping and one lossless wake endpoint through native teardown.
mod messages;
#[cfg(test)]
mod race_tests;
#[cfg(test)]
mod tests;
mod wake;
#[allow(unsafe_code)]
mod windows;
use crate::DesktopError;
use std::{cell::Cell, marker::PhantomData, rc::Rc, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError};
pub(crate) use wake::BootstrapWake;

/// Native primitive facts; neither a wake nor a deadline proves owner settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaitOutcome {
    Woken,
    Deadline,
    Quit(i32),
}

/// Isolate actual event and main-thread message operations, not application preparation.
trait EventBoundary: Send + Sync {
    fn signal(&self) -> Result<(), ExecutionError>;
    fn reset(&self) -> Result<(), ExecutionError>;
    fn wait(&self, deadline: Option<Instant>, use_event: bool)
    -> Result<WaitOutcome, DesktopError>;
    fn close(&self) -> Result<(), DesktopError>;
}

/// Stay on the creating main thread while workers retain only the synchronized wake.
/// Failed native event operations retain custody and switch closing waits to messages;
/// final checked closure remains explicit and never follows a dropped job/future.
pub(crate) struct Bootstrap {
    event: Arc<dyn EventBoundary>,
    wake: Arc<BootstrapWake>,
    failed: Cell<bool>,
    affinity: PhantomData<Rc<()>>,
}
impl Bootstrap {
    /// Create the native completion event before any fallible content start or Tao loop.
    pub(crate) fn new() -> Result<Self, DesktopError> {
        Ok(Self::from_event(Arc::new(windows::NativeEvent::new()?)))
    }
    fn from_event(event: Arc<dyn EventBoundary>) -> Self {
        Self {
            wake: Arc::new(BootstrapWake::new(event.clone())),
            event,
            failed: Cell::new(false),
            affinity: PhantomData,
        }
    }
    /// Share exactly the same endpoint for this process's one execution owner.
    pub(crate) fn wake(&self) -> Arc<dyn CompletionWake> {
        self.wake.clone()
    }
    /// Clear the event before a bounded owner drain, so racing arrivals remain signalled.
    pub(crate) fn begin_turn(&self) -> Result<(), DesktopError> {
        if self.failed.get() {
            return Ok(());
        }
        self.wake.begin_turn().map_err(|cause| {
            self.failed.set(true);
            DesktopError::new("bootstrap_reset", cause)
        })
    }
    /// Wait on the current absolute deadline and dispatch at most 32 queued native messages.
    pub(crate) fn wait(&self, deadline: Option<Instant>) -> Result<WaitOutcome, DesktopError> {
        self.event.wait(deadline, !self.failed.get()).inspect_err(|_| self.failed.set(true))
    }
    /// Publish the real native proxy through the same endpoint, then close the old event.
    /// A failed new wake leaves the original event selected; checked-close failure after
    /// publication retains the actual new target and old event for final retirement.
    pub(crate) fn handoff(&self, target: Arc<dyn CompletionWake>) -> Result<(), DesktopError> {
        self.wake
            .retarget(target)
            .map_err(|cause| DesktopError::new("bootstrap_handoff", cause))?;
        self.close_event()
    }
    fn close_event(&self) -> Result<(), DesktopError> {
        self.event.close().map_err(|cause| {
            DesktopError::new(
                "bootstrap_event_retirement",
                RetirementFailure { cause, event: self.event.clone() },
            )
        })
    }
    /// Preserve native custody even when an earlier application failure already exists.
    /// This finite teardown boundary retains the primary cause and the actual failed
    /// retirement together; secondary diagnostic logging cannot dispose native ownership.
    pub(crate) fn retain_close_failure(&self, failure: &mut Option<DesktopError>) {
        if let Err(retirement) = self.disconnect() {
            *failure = Some(match failure.take() {
                Some(primary) => DesktopError::new(
                    "bootstrap_retained_cleanup",
                    RetainedCleanup { primary, retirement },
                ),
                None => retirement,
            });
        }
    }
    /// Fence every worker wake before checked event closure, after actual owner settlement.
    pub(crate) fn disconnect(&self) -> Result<(), DesktopError> {
        self.wake.disconnect().map_err(|cause| {
            DesktopError::new(
                "bootstrap_disconnect",
                RetirementFailure {
                    cause: DesktopError::new("bootstrap_wake_fence", cause),
                    event: self.event.clone(),
                },
            )
        })?;
        self.close_event()
    }
}

/// Return the actual native construction or its retained original panic payload.
/// Preparation and cleanup remain with the already-started composing owner.
pub(crate) fn construct_loop<T>(construct: impl FnOnce() -> T) -> Result<T, DesktopError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(construct)).map_err(|payload| {
        DesktopError::new("bootstrap_tao_construction", ConstructionFailure(payload))
    })
}
struct ConstructionFailure(Box<dyn std::any::Any + Send>);
impl std::fmt::Debug for ConstructionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConstructionFailure").field("text_payload", &self.0.is::<String>()).finish()
    }
}
impl std::fmt::Display for ConstructionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Native event-loop construction panicked")
    }
}
impl std::error::Error for ConstructionFailure {}

/// Preserve actual native event custody with a returned failure, independently of
/// cancelled execution/teardown. A later checked close of the same event may settle
/// it; observing/dropping this error never establishes native closure.
struct RetirementFailure {
    cause: DesktopError,
    event: Arc<dyn EventBoundary>,
}
impl std::fmt::Debug for RetirementFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetirementFailure")
            .field("cause", &self.cause)
            .field("retained_event_owners", &Arc::strong_count(&self.event))
            .finish()
    }
}
impl std::fmt::Display for RetirementFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for RetirementFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Keep the first error and actual native retirement ownership in one finite result.
#[derive(Debug)]
struct RetainedCleanup {
    primary: DesktopError,
    retirement: DesktopError,
}
impl std::fmt::Display for RetainedCleanup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}; {}", self.primary, self.retirement)
    }
}
impl std::error::Error for RetainedCleanup {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}
