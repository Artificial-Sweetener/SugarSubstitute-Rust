// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe owned engine handoff and retire its entire containment before accepted fallback.
use super::*;

impl Launcher {
    /// Pace nonblocking native observations through the existing event-loop deadline owner.
    pub(super) fn activation_due(&self) -> bool {
        self.activation_after.is_none_or(|after| self.clock.now() >= after)
    }

    /// Observation uses admitted workers and the original startup or cleanup cutoff.
    pub(super) fn observe_activation(&mut self, stop: bool) -> Result<()> {
        let activation = self.activation.as_ref().ok_or("Activation custody absent")?.clone();
        self.activation_after = None;
        self.activation_wake = None;
        self.submit(if stop { Task::StopEngine } else { Task::ObserveEngine }, move |context| {
            activation.poll(&context, stop).map(Work::EngineObserved).map_err(io::Error::other)
        })
    }

    /// A failed engine creation/query never authorizes an immediate second application process.
    pub(super) fn activation_failed(&mut self, cause: Box<dyn Error>) -> Result<()> {
        tracing::warn!(operation="startup_engine_failed", cause=%crate::failure_details::Details(cause.as_ref()), "Settling activation containment before accepted fallback");
        self.update_failure.get_or_insert(cause);
        let now = self.clock.now();
        let until = sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(now)?
            .settle_until
            .min(self.until.ok_or("Launcher startup cutoff absent")?);
        let until = *self.recovery_until.get_or_insert(until);
        self.recovery_wake =
            Instant::now().checked_add(until.checked_duration_since(now).unwrap_or_default());
        self.activation_after = None;
        self.activation_wake = None;
        self.choice = Choice::StopEngine;
        Ok(())
    }

    /// Actual transfer is durable in the native owner even if an earlier job result was cancelled.
    /// Actual zero membership permits accepted selection, which rechecks native installation state.
    pub(super) fn activation_observed(&mut self, outcome: ActivationOutcome) -> Result<()> {
        if let ActivationOutcome::Settling { exit_code } = outcome
            && !self.closing
            && self.recovery_until.is_none()
        {
            self.activation_failed(format!("Activation engine exited with {exit_code:?}").into())?;
        }
        if matches!(outcome, ActivationOutcome::Pending | ActivationOutcome::Settling { .. }) {
            return self.pace_activation();
        }
        self.activation = None;
        self.activation_after = None;
        self.activation_wake = None;
        self.recovery_until = None;
        self.recovery_wake = None;
        if self.closing {
            return Ok(());
        }
        match outcome {
            ActivationOutcome::Stopped { creation: CreationOutcome::Created, .. } => {
                self.completed = true;
                self.choice = Choice::Complete;
            }
            ActivationOutcome::Stopped { exit_code, creation: CreationOutcome::NotCreated } => {
                tracing::warn!(
                    operation = "startup_engine_settled",
                    exit_code,
                    "Activation containment settled; selecting accepted application"
                );
                self.choice = Choice::Accepted;
            }
            ActivationOutcome::Stopped { creation: CreationOutcome::Indeterminate, .. } => {
                return Err(self
                    .update_failure
                    .take()
                    .unwrap_or_else(|| "Native application creation remains uncertain".into()));
            }
            ActivationOutcome::Pending | ActivationOutcome::Settling { .. } => {}
        }
        Ok(())
    }
}
