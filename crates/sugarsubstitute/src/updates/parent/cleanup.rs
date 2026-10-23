// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close only the owned preparation family and observe its actual resource retirement.
use super::*;
impl Parent {
    /// Freeze cleanup once. Closing the application supplies its existing shutdown budget.
    pub(crate) fn close(&mut self, budget: Option<ShutdownBudget>) -> Result<()> {
        if self.shutdown.is_some() || self.retired {
            return Ok(());
        }
        let now = self.client.now()?;
        let start = self.continuation.as_ref().map_or(now, |window| now.min(window.finish_until));
        let budget = budget.map(Ok).unwrap_or_else(|| ShutdownBudget::new(start))?;
        budget.validate()?;
        let anchor = Instant::now();
        self.grace_wake = anchor
            .checked_add(budget.graceful_until.checked_duration_since(now).unwrap_or_default());
        self.settle_wake =
            anchor.checked_add(budget.settle_until.checked_duration_since(now).unwrap_or_default());
        self.shutdown = Some(budget);
        self.work.cancel();
        if let Some(registry) = self.retained.as_mut().and_then(|held| held.registry.as_mut()) {
            registry.close_launch_admission();
        }
        Ok(())
    }

    /// No native record, input or borrowed application guard is dropped on the pump thread.
    pub(super) fn retire(&mut self, jobs: &JobSequence, now: Moment) -> Result<()> {
        if self.retired
            || self.retiring.is_some()
            || self.constructing.is_some()
            || self.preparing.is_some()
        {
            return Ok(());
        }
        let budget = self.shutdown.ok_or("Update cleanup budget absent")?;
        if now >= budget.settle_until {
            return Ok(());
        }
        if let Some(process) = self.process {
            if !self.stop_issued {
                self.retained
                    .as_mut()
                    .and_then(|held| held.registry.as_mut())
                    .ok_or("Update registry absent")?
                    .begin_stop(process, ShutdownId::new(1)?, budget)?;
                self.stop_issued = true;
            }
            let registry = self
                .retained
                .as_ref()
                .and_then(|held| held.registry.as_ref())
                .ok_or("Update registry absent")?;
            let snapshot = registry.snapshot(process)?;
            if snapshot.settled {
                self.retained
                    .as_mut()
                    .and_then(|held| held.registry.as_mut())
                    .ok_or("Update registry absent")?
                    .retire_settled(process)?;
                self.process = None;
            } else if snapshot.cleanup_step_due(now) {
                let watch = if snapshot.member_watch_required {
                    Some(self.spec(jobs, JobClass::NativeObservation, 4096, None)?)
                } else {
                    None
                };
                let spec = self.spec(
                    jobs,
                    JobClass::InfrastructureLifecycle,
                    4096,
                    Some(budget.settle_until),
                )?;
                self.retained
                    .as_mut()
                    .and_then(|held| held.registry.as_mut())
                    .ok_or("Update registry absent")?
                    .submit_cleanup_step(&self.client, process, &self.cleanup, spec, watch)?;
            }
        }
        if self.process.is_some()
            || self
                .retained
                .as_ref()
                .and_then(|held| held.registry.as_ref())
                .is_some_and(|registry| !registry.is_empty())
        {
            return Ok(());
        }
        if self.retained.is_some() {
            let spec = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                4096,
                Some(budget.settle_until),
            )?;
            let application = self.retained.as_ref().is_some_and(|held| held.source.is_some());
            self.retiring = Some(self.client.submit_blocking_owned_input(
                &self.cleanup,
                spec,
                &mut self.retained,
                |held, _context| {
                    drop(held);
                    Ok(())
                },
            )?);
            self.retiring_application = application;
        }
        Ok(())
    }
}
