// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deliver only the authoritative frontend expiry through the existing failed-attempt owner.
use super::*;
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_contracts::startup::{StartupFailure, StartupFailureCode};

impl ProductionChild {
    /// Publish one actual expiry without changing its cutoff, issuing a job or claiming cleanup.
    /// Slot rejection retains the same original error as infrastructure custody.
    pub(super) fn publish_frontend_timeout(&mut self, cause: std::io::Error) -> Result<()> {
        if self.frontend_timed_out {
            return Ok(());
        }
        let cause: Box<dyn Error> = Box::new(cause);
        if self.events.iter().all(Option::is_some) {
            if self.failure.is_none() {
                self.failure = Some(cause);
            }
            return Err("Frontend failure event slots full; original cause retained".into());
        }
        self.push_event(StartupEvent::new(
            self.identity.launch,
            StartupObservation::Failed(StartupFailure::new(StartupFailureCode::TimedOut, cause)),
        ))?;
        self.frontend_timed_out = true;
        self.failed = true;
        Ok(())
    }
    /// Check exact native evidence admission against the same original frontend budget.
    /// An existing attempt cleanup or shutdown keeps ownership of its original transaction.
    /// # Errors
    /// Preserve clock or bounded failure-publication rejection; never classify arbitrary errors.
    pub(in crate::production) fn expire_frontend(&mut self, phase: StartupPhase) -> Result<bool> {
        if self.frontend_timed_out {
            return Ok(true);
        }
        match connection::ActivityRoute::select(
            self.shutdown.closing(),
            self.attempt.as_ref(),
            Some(phase),
            self.frontend_budget,
            self.client()?.now()?,
        ) {
            Ok(_) => Ok(false),
            Err(cause) => {
                self.publish_frontend_timeout(cause)?;
                Ok(true)
            }
        }
    }
}
#[cfg(test)]
#[path = "frontend_deadline_tests.rs"]
pub(super) mod tests;
