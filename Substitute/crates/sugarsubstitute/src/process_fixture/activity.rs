// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance bounded typed registry observations without waiting or inventing native facts.
use super::*;
use sugarsubstitute_desktop::HeadlessStep;

impl Fixture {
    pub(super) fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.drain(max)?;
        self.watch.check()?;
        if !self.closing
            && self.registry_creation.is_none()
            && self.registry.is_some()
            && self.current < self.cases.len()
            && self.cases[self.current].process.is_none()
        {
            self.launch_next()?;
        }
        if self.current == self.cases.len() {
            return Ok(HeadlessStep::Shutdown);
        }
        let Some(key) = self.cases[self.current].process else {
            return Ok(if self.closing { HeadlessStep::Shutdown } else { HeadlessStep::Active });
        };
        // Each record has five unread fact classes; no dynamic event list is retained.
        for _ in 0..5 {
            let Some(observation) = self
                .registry
                .as_mut()
                .ok_or("Actual registry is not prepared")?
                .take_observation(key)?
            else {
                break;
            };
            self.watch.progressed();
            self.cases[self.current].observe(observation)?;
        }
        if self.cases[self.current].exited.is_some() || self.closing {
            self.ensure_stop()?;
        }
        let snapshot =
            self.registry.as_ref().ok_or("Actual registry is not prepared")?.snapshot(key)?;
        if let Some(stop) = &mut self.stop {
            let due = stop.due(self.clock.now(), snapshot.settled);
            self.cleanup_requested |= due.cleanup;
            if due.expired {
                return Err("Registry qualification family remains unsettled after its original fifteen-second budget".into());
            }
        }
        if self.cases[self.current].settled {
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.retire_settled(key)?;
            self.cases[self.current].retired = true;
            if !self.closing && !self.cases[self.current].complete() {
                return Err(
                    "Registry qualification lacks actual startup/exit/settlement evidence".into()
                );
            }
            self.current += 1;
            self.stop = None;
            self.cleanup_job = None;
            self.cleanup_requested = false;
            if !self.closing {
                self.launch_next()?;
            }
        } else if self.stop.is_some()
            && !snapshot.creation_pending
            && self.cleanup_job.is_none()
            && self.cleanup_requested
        {
            self.submit_cleanup(key, snapshot.member_watch_required)?;
        }
        Ok(if self.closing || self.current == self.cases.len() {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
    fn submit_cleanup(
        &mut self,
        key: sugarsubstitute_contracts::lifecycle::ProcessKey,
        watch_required: bool,
    ) -> Result<()> {
        let watch =
            if watch_required { Some(self.spec(JobClass::NativeObservation, 4096)?) } else { None };
        let step = self.spec(JobClass::InfrastructureLifecycle, 4096)?;
        match self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_cleanup_step(
            self.client.as_ref().ok_or("Fixture execution client missing")?,
            key,
            self.scope.as_ref().ok_or("Fixture cleanup scope missing")?,
            step,
            watch,
        ) {
            Ok(id) => {
                self.cleanup_job = Some(id);
                self.cleanup_requested = false;
                Ok(())
            }
            Err(error) => {
                // Retain the original admission cause and family. Retry follows an
                // observed job completion or the original grace/total boundary,
                // rather than the native pump's Closing cadence.
                self.cleanup_requested = false;
                Err(Box::new(error))
            }
        }
    }
}
