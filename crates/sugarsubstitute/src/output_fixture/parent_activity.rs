// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact drained facts before fallible processing and bound every parser turn.
use super::{Result, parent::Parent};
use std::{num::NonZeroUsize, time::Instant};
use sugarsubstitute_contracts::execution::ExecutionPort;
use sugarsubstitute_desktop::HeadlessStep;
impl Parent {
    pub(super) fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let batch = self
            .client
            .as_ref()
            .ok_or("Output parent execution missing")?
            .drain_completions(max)?;
        for case in &mut self.cases {
            case.capture(&batch)?;
        }
        let _table_changed = crate::process_fixture::registry::poll(
            &mut self.registry_creation,
            &mut self.registry,
            self.closing,
        )?;
        if let Some(registry) = &mut self.registry {
            registry.consume(&batch)?;
        }
        if self.registry.is_none() {
            if !self.deadline_observed && Instant::now() >= self.deadline {
                self.deadline_observed = true;
                return Err("Registry construction exceeded original qualifier deadline".into());
            }
            return Ok(if self.closing {
                sugarsubstitute_desktop::HeadlessStep::Shutdown
            } else {
                sugarsubstitute_desktop::HeadlessStep::Active
            });
        }
        if self.current == self.cases.len() {
            return Ok(HeadlessStep::Shutdown);
        }
        self.cases[self.current].consume_jobs(self.closing)?;
        self.control(&batch)?;
        if !self.deadline_observed && Instant::now() >= self.deadline {
            self.deadline_observed = true;
            return Err("Output qualification reached original60s deadline".into());
        }
        let case = &self.cases[self.current];
        if !self.closing
            && case.process.is_none()
            && case.store.is_some()
            && case.output.as_ref().is_some_and(|s| s.control.snapshot().prepared)
            && case.channel.as_ref().is_none_or(|c| c.control().prepared_run() == Some(case.run))
        {
            self.launch()?;
        }
        let moved = self.cases[self.current].drain_output(self.closing)?;
        // Actual queued payload supplies this bounded continuation. There is no
        // wake while a flood loan is intentionally held or an idle pipe is parked.
        if moved && let Some(wake) = &self.wake {
            wake.wake()?;
        }
        self.control(&sugarsubstitute_contracts::execution::CompletionBatch {
            notifications: Vec::new(),
            remaining: 0,
        })?;
        self.progress_family()?;
        let case = &self.cases[self.current];
        if case.retired && case.drained && case.channel.as_ref().is_none_or(|c| c.terminal) {
            if !self.closing && !case.complete() {
                return Err("Output case lacks all independent actual evidence".into());
            }
            self.cases[self.current].output = None;
            self.current += 1;
            self.reserve_next()?;
        }
        Ok(if self.closing || self.current == self.cases.len() {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
}
