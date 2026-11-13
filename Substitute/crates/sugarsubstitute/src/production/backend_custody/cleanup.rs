// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the authenticated capture's first cleanup cutoff and original observation cause.
use std::error::Error;
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_contracts::lifecycle::ShutdownBudget;
use sugarsubstitute_execution::{
    windows::NativeObservationError, windows_capture_attach::CaptureCleanupSnapshot,
};

/// Cached capture facts precede any global execution shutdown admission.
#[derive(Default)]
pub(super) struct Cleanup {
    budget: Option<ShutdownBudget>,
    cause: Option<NativeObservationError>,
}
impl Cleanup {
    /// Adopt the first real transaction before reporting a native counter failure.
    /// Requested cleanup stays pending; failed measurement cannot acquire a replacement cutoff.
    pub(super) fn observe(
        &mut self,
        snapshot: CaptureCleanupSnapshot,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(cause) = &self.cause {
            return Err(Box::new(cause.clone()));
        }
        if self.budget.is_some() {
            return Ok(());
        }
        match snapshot {
            CaptureCleanupSnapshot::Requested => Ok(()),
            CaptureCleanupSnapshot::Dormant => {
                Err("Helper capture cleanup was not requested".into())
            }
            CaptureCleanupSnapshot::Latched { budget, counter } => {
                self.budget = Some(budget);
                if let Err(cause) = counter {
                    self.cause = Some(cause.clone());
                    return Err(Box::new(cause));
                }
                Ok(())
            }
            CaptureCleanupSnapshot::ClockUnavailable(cause) => {
                self.cause = Some(cause.clone());
                Err(Box::new(cause))
            }
        }
    }
    /// Return the original execution cutoff even when its native counter conversion failed.
    pub(super) const fn until(&self) -> Option<Moment> {
        match self.budget {
            Some(budget) => Some(budget.settle_until),
            None => None,
        }
    }
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod tests;
