// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original earliest parent cutoff without treating cancellation as settlement.
use super::*;
impl IncidentHost {
    /// Close new admission and tighten the same original shutdown budget.
    pub(in crate::production) fn request_close(&mut self, budget: ShutdownBudget) {
        self.closing = true;
        self.shutdown = Some(match self.shutdown {
            Some(old) => ShutdownBudget {
                started: old.started.min(budget.started),
                graceful_until: old.graceful_until.min(budget.graceful_until),
                settle_until: old.settle_until.min(budget.settle_until),
            },
            None => budget,
        });
        if let Some(job) = &self.constructor {
            job.cancel();
        }
        if let Some(JobOutcome::Success(controller)) =
            self.completion.as_ref().map(|value| &value.outcome)
        {
            controller.policy.recovery.cancel();
            controller.policy.seed.cancel();
            controller.policy.collection.cancel();
            controller.policy.reporting.cancel();
            controller.policy.exporting.cancel();
        }
    }
    /// Borrow close-only seed custody after admission closure; no new seed can start here.
    pub(in crate::production) fn retiring_seed_service(
        &mut self,
    ) -> Result<&mut super::super::diagnostic_helper::SeedService, HostIssue> {
        if !self.closing {
            return Err(HostIssue::Identity);
        }
        match self.completion.as_mut().map(|value| &mut value.outcome) {
            Some(JobOutcome::Success(controller)) => Ok(&mut controller.policy.seed),
            _ => Err(HostIssue::Unprepared),
        }
    }
    /// Inspect the constructor's actual terminal transfer; expiry supplies no proof.
    /// Later genuine helper/service owners must settle independently before final disposal.
    pub(in crate::production) fn construction_settled(&self) -> bool {
        self.closing && self.construction_delivered()
    }
    /// Inspect the actual constructor transfer while an original recovery use case is finishing.
    pub(in crate::production) fn construction_delivered(&self) -> bool {
        self.constructor.is_none()
    }
    /// Inspect actual constructor, queued effects and policy duties without closing admission.
    /// The namespace/controller remains retained while ordinary close settles its owners.
    pub(in crate::production) fn diagnostic_custody_pending(&self) -> bool {
        if self.constructor.is_some() {
            return true;
        }
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(controller)) => {
                controller.policy.state.as_ref().is_none_or(|state| {
                    controller.policy.recovery.pending()
                        || state.state.effects.iter().any(Option::is_some)
                        || state.state.coordinator.diagnostic_custody_pending()
                })
            }
            Some(_) => true,
            None => false,
        }
    }
    /// Observe admission closure without deriving any file/native retirement proof.
    pub(in crate::production) const fn closing(&self) -> bool {
        self.closing
    }
    /// Route actual helper terminals during both normal entry and bounded shutdown.
    /// This method supplies no native, process or file settlement from an expired budget.
    pub(in crate::production) fn capture_seed_jobs(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, super::super::diagnostic_helper::SeedServiceIssue> {
        let Some(JobOutcome::Success(controller)) =
            self.completion.as_mut().map(|value| &mut value.outcome)
        else {
            return Ok(false);
        };
        let seed = &mut controller.policy.seed;
        let main = seed.capture(batch)?;
        Ok(seed.capture_cleanup(batch)? || main)
    }
}
