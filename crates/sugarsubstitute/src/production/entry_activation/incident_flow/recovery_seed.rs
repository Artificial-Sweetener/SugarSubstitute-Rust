// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire one failed recovery run's genuine seed work without closing shared recovery or clients.
use super::*;
impl IncidentFlow {
    /// Report only a matching seed duty; unrelated current work cannot be claimed or cancelled.
    pub(in crate::production::entry_activation) fn recovery_seed_pending(
        &self,
        run: RunKey,
    ) -> bool {
        self.seed.as_ref().is_some_and(|active| active.run == run)
    }
    /// Cancel one actual same-run worker and observe its terminal before checked close-only cleanup.
    /// Successful publication is adopted normally. Failed/cancelled publication retains all exact
    /// originals in the returned owner after the typed same-scope policy retirement is accepted.
    /// The first cleanup cutoff remains unchanged; this never closes the shared recovery flow.
    /// During global close, the retiring controller retains the originals after checked cleanup.
    pub(in crate::production::entry_activation) fn close_recovery_seed(
        &mut self,
        run: RunKey,
        resources: &EntryResources<'_>,
        budget: ShutdownBudget,
    ) -> Result<Option<RetiredRecoverySeed>> {
        let Some(active) = self.seed.as_mut().filter(|active| active.run == run) else {
            return Ok(None);
        };
        let scope = active.scope;
        let globally_closing = self.host.closing();
        let service = if globally_closing {
            self.host.retiring_seed_service()?
        } else {
            self.host.seed_service()?
        };
        service.cancel();
        if active.submitted && service.outcome().is_none() {
            return Ok(None);
        }
        let until = *active.cleanup_until.get_or_insert(budget.settle_until);
        if !service.cleanup_started() {
            match service.submit_cleanup(resources.client, resources.cleanup, resources.jobs, until)
            {
                Ok(()) => active.cleanup_submitted = true,
                Err(SeedServiceIssue::Admission) => return Ok(None),
                Err(cause) => return Err(cause.into()),
            }
        }
        if !service.files_retired() {
            return Ok(None);
        }
        if globally_closing {
            // The closed controller keeps the actual results and native/data credit.
            // Global shutdown has no live policy admission to adopt or detach here.
            self.seed = None;
            return Ok(None);
        }
        if service.publication_succeeded() {
            self.host.adopt_seed_service(scope)?;
            self.seed = None;
            return Ok(None);
        }
        let retired = self.host.retire_failed_seed_service(scope)?;
        self.seed = None;
        Ok(Some(retired))
    }
}
