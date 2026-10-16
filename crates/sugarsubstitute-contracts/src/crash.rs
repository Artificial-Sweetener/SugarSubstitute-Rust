// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe native capture observations without granting process or filesystem authority.
//!
//! Capture registration is separate from M1 exit classification and resource
//! settlement. A handler identity is a kernel matching fact, never a capability.
use crate::{
    execution::{Moment, TimeError},
    lifecycle::CoordinatorKey,
};
use std::time::Duration;

/// State of the supervisor's single process-lifetime capture registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureState {
    /// No native registration has been attempted.
    Dormant,
    /// Admitted finite start has not transferred its terminal observation.
    Starting,
    /// Native client registration succeeded with retained, watched handler ownership.
    Registered,
    /// Capture or its observation failed; retained native owners remain explicit.
    Degraded,
    /// Explicit logical shutdown is observing handler/native retirement.
    Closing,
    /// Actual handler/native retirement completed; installed callbacks remain process-pinned.
    Closed,
}

/// Categorical failed operation suitable for bounded product observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureOperation {
    /// Verify and load the exact trusted runtime bundle.
    Runtime,
    /// Register this client and retain the actual handler.
    Registration,
    /// Bind or consume actual handler exit observation.
    Observation,
    /// Request handler termination after the caller's capture policy permits it.
    Stop,
    /// Observe and close native partial outputs.
    Retirement,
}

/// Bounded failure category; original implementation causes stay with the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureFailureCode {
    /// A native operation failed or left uncertain retained ownership.
    Native,
    /// The admitted operation was cancelled after actual work stopped.
    Cancelled,
    /// Its original admitted deadline elapsed.
    TimedOut,
    /// Supervised work panicked.
    Panicked,
    /// The actual handler observation did not match retained kernel identity.
    IdentityMismatch,
}

/// Actual retained handler identity; fields contain no process-opening authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureHandlerIdentity {
    /// Kernel PID read from the retained handle.
    pub process_id: u32,
    /// Creation time in 100ns units since January 1, 1601 UTC.
    pub creation_time_100ns: u64,
}

/// One bounded capture fact; domain values contain no native handle/error type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureObservation {
    /// Actual native registration and ownership transfer succeeded.
    Registered {
        /// Supervisor lifetime being observed.
        owner: CoordinatorKey,
        /// Actual retained and watched handler.
        handler: CaptureHandlerIdentity,
    },
    /// Capture is degraded independently of M1's exit/resource observations.
    Degraded {
        /// Supervisor lifetime being observed.
        owner: CoordinatorKey,
        /// Failed native/execution boundary.
        operation: CaptureOperation,
        /// Redacted categorical reason.
        cause: CaptureFailureCode,
    },
    /// The watched kernel process actually signalled with this exit code.
    HandlerExited {
        /// Supervisor lifetime being observed.
        owner: CoordinatorKey,
        /// Retained handler identity matched by the actual watch.
        handler: CaptureHandlerIdentity,
        /// Actual native exit code, including zero and 259.
        exit_code: u32,
    },
    /// All explicit handler/registration owners retired; callback module stays pinned.
    Closed {
        /// Supervisor lifetime being observed.
        owner: CoordinatorKey,
    },
}

/// One original 60-second capture-operation budget in an execution-clock epoch.
/// Registration and later fatal capture use separate original budget instances;
/// neither replaces the normal 15-second shutdown budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureBudget {
    /// Original moment in the caller's execution-clock epoch.
    started: Moment,
    /// The original operation deadline, exactly 60 seconds later.
    until: Moment,
}
impl CaptureBudget {
    /// Retain one original deadline without resetting it on retries.
    ///
    /// # Errors
    /// Returns `TimeError::Overflow` before constructing an invalid budget.
    pub fn new(started: Moment) -> Result<Self, TimeError> {
        Ok(Self { started, until: started.checked_add(Duration::from_secs(60))? })
    }
    /// Return the original start in the owning execution clock's nanosecond epoch.
    pub const fn started(self) -> Moment {
        self.started
    }
    /// Return the immutable checked 60-second deadline in that same epoch.
    pub const fn until(self) -> Moment {
        self.until
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fatal_budget_is_exact_and_overflow_is_rejected() -> Result<(), TimeError> {
        let budget = CaptureBudget::new(Moment::from_nanos(17))?;
        assert_eq!(budget.started.as_nanos(), 17);
        assert_eq!(budget.until.as_nanos(), 60_000_000_017);
        assert_eq!(CaptureBudget::new(Moment::from_nanos(u64::MAX)), Err(TimeError::Overflow));
        Ok(())
    }
}

mod target;
pub use target::CaptureTarget;

mod origin;
pub use origin::{CaptureGenerationOrigin, CaptureOriginNameError, RecoveryCaptureIdentity};
