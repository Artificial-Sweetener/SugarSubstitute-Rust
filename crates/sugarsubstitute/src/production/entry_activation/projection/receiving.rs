// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt actual projection completions into charged records before parent construction.
use super::super::*;
impl ProductionActivation {
    pub(in crate::production::entry_activation) fn receive_projections(
        &mut self,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        for pending in self.pending.iter_mut().flatten() {
            if let Some(job) = &mut pending.job
                && let Some(completion) = job.try_take_completion()
            {
                pending.completion = Some(completion);
                pending.job = None;
            }
        }
        if self.closing {
            for pending in self.pending.iter_mut().flatten() {
                if let Some(completed) = &mut pending.completion {
                    projection::return_unused_source(&mut completed.outcome);
                }
            }
        }
        for at in 0..self.pending.len() {
            if !self.closing
                && let Some(pending) = &self.pending[at]
            {
                match reactivation::reuse(&self.records, pending, resources)? {
                    reactivation::Reuse::Accepted | reactivation::Reuse::Expired => {
                        self.pending[at] = None;
                        continue;
                    }
                    reactivation::Reuse::Waiting => continue,
                    reactivation::Reuse::New => {}
                }
            }
            let Some(pending) = &mut self.pending[at] else {
                continue;
            };
            let Some(completion) = &mut pending.completion else {
                continue;
            };
            if self.backend.is_none()
                && let JobOutcome::Success(value) = &mut completion.outcome
            {
                let storage = completion.memory.claim(RetainedBackend::required_storage_bytes())?;
                self.backend = Some(
                    RetainedBackend::new(resources.authority, storage, &mut value.policy_credit)
                        .map_err(outer)?,
                );
                self.backend
                    .as_mut()
                    .ok_or("Backend owner absent")?
                    .backend
                    .configure_custody(self.executable.clone(), self.working_directory.clone())
                    .map_err(outer)?;
            }
            if let JobOutcome::Success(value) = &mut completion.outcome
                && value.policy_credit.is_some()
            {
                self.backend
                    .as_mut()
                    .ok_or("Actual policy funding owner missing")?
                    .add_projection_credit(&mut value.policy_credit)
                    .map_err(outer)?;
            }
            if self.delivery.is_none() && matches!(completion.outcome, JobOutcome::Success(_)) {
                let storage =
                    completion.memory.claim(delivery::TransitionDelivery::storage_bytes())?;
                self.delivery = Some(delivery::TransitionDelivery::new(storage)?);
            }
            let storage = completion.memory.claim(size_of::<Record>() as u64)?;
            let projected =
                pending.completion.take().ok_or("Actual projection completion missing")?;
            let replied = pending.restart
                && self.records[at].as_ref().is_some_and(|held| held.record.replied);
            let mut record = Record::new(
                sugarsubstitute_contracts::startup::MainIdentity {
                    run: pending.run,
                    launch: pending.launch,
                    navigation: pending.navigation,
                },
                pending.connection,
                std::mem::take(&mut pending.qualification),
                replied,
                pending.restart,
            );
            if let JobOutcome::Success(value) = &projected.outcome {
                self.selected_locale.get_or_insert(value.locale);
            }
            record.projected = Some(projected);
            let mut successor = RetainedRecord { record: Box::new(record), _storage: storage };
            if let Some(previous) = self.records[at].as_mut() {
                cause_transfer::before_replace(
                    &mut previous.record,
                    &mut successor.record,
                    pending.restart,
                )?;
            }
            self.records[at] = Some(successor);
            self.pending[at] = None;
        }
        self.check_initial_projection()
    }

    /// Observe failed initial preparation before incident recovery requires its successful source.
    pub(in crate::production::entry_activation) fn check_initial_projection(
        &mut self,
    ) -> Result<()> {
        if self.backend.is_none()
            && let Some(record) =
                self.records.iter().flatten().map(|held| held.record.as_ref()).find(|record| {
                    record.projected.as_ref().is_some_and(|completed| {
                        !matches!(completed.outcome, JobOutcome::Success(_))
                    })
                })
        {
            self.failure_run.get_or_insert(record.run);
            return Err("Actual entry projection failed with retained original outcome".into());
        }
        Ok(())
    }
}
