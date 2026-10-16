// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve failed local recovery custody while unrelated Root clients keep running.
use super::*;
impl RecoveryPreparation {
    pub(super) fn close(
        &mut self,
        root: &mut ProductionActivation,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if self.installed {
            return Ok(());
        }
        if self.cleanup.is_none() {
            self.cleanup = Some(ShutdownBudget::new(resources.client.now()?)?);
        }
        if let Some(job) = &self.job {
            job.cancel();
        }
        if let Some(completed) = &mut self.completion {
            super::super::projection::return_unused_source(&mut completed.outcome);
        }
        let mut first = None;
        if let Some(record) = &self.record {
            record.record.seed.cancel();
        }
        let budget = self.cleanup.ok_or("Original recovery cleanup budget missing")?;
        match root.incidents.close_recovery_seed(self.identity.run, resources, budget) {
            Ok(Some(retired)) => self.retired_seed = Some(retired),
            Ok(None) => {}
            Err(cause) => {
                first.get_or_insert(cause);
            }
        }
        self.seed_settled = !root.incidents.recovery_seed_pending(self.identity.run);

        if !self.ack_requested && !self.rejected && root.closing {
            match root
                .incidents
                .reporting
                .dispose_deferred_fresh_restart_after_retirement(self.selected.binding())
            {
                Ok(retired) => self.rejected = retired,
                Err(cause) => {
                    first.get_or_insert(cause);
                }
            }
        } else if !self.ack_requested
            && !self.rejected
            && root.incidents.reporting.deferred_fresh_restart().is_some()
        {
            match root.incidents.reporting.reject_deferred_fresh_restart() {
                Ok(()) => self.rejected = true,
                Err(cause) => {
                    first.get_or_insert(cause);
                }
            }
        }
        if let Some(origin) = &mut self.origin
            && let Err(cause) = origin.close(resources.client, resources.cleanup, resources.jobs)
        {
            first.get_or_insert(Box::new(cause));
        }
        if let Some(record) = &mut self.record {
            if let Some(start) = &mut record.record.start
                && let Err(cause) = start.close_recovery(resources, &budget)
            {
                first.get_or_insert(cause);
            }
            if let Some(backend) = &mut root.backend
                && let Err(cause) = ProductionActivation::close_record(
                    record.record.as_mut(),
                    backend.backend.as_mut(),
                    root.capture.as_ref(),
                    resources,
                    budget,
                )
            {
                first.get_or_insert(cause);
            }
        }
        if let Some(grant) = &self.grant {
            grant.close_admission();
        }
        first.map_or(Ok(()), Err)
    }
    pub(in crate::production::entry_activation) fn settled(&self) -> bool {
        self.installed
            || (self.cleanup.is_some()
                && (self.ack_requested || self.rejected)
                && self.job.is_none()
                && self.seed_settled
                && self.record.as_ref().is_none_or(|held| {
                    held.record.closed
                        && held.record.seed.delivered()
                        && held.record.collection.delivered()
                        && held.record.start.as_ref().is_none_or(|start| {
                            start.recovery_settled()
                                && !start.profile.snapshot().pending
                                && start.profile.snapshot().retirement_ready()
                        })
                })
                && self.origin.as_ref().is_none_or(|origin| origin.files_retired()))
    }
}
