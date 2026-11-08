// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reproject the genuine registered archive once and retain the successful grant/completion.
use super::*;
use sugarsubstitute_contracts::execution::AdmissionError;
use sugarsubstitute_execution::OwnedInputError;
impl RecoveryPreparation {
    pub(super) fn prepare_projection(
        &mut self,
        root: &mut ProductionActivation,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if self.job.is_some() {
            return Ok(());
        }
        if self.completion.is_none() {
            let old = root.records[self.at]
                .as_ref()
                .ok_or("Actual recovery source slot missing")?
                .record
                .as_ref();
            if old.run != self.previous {
                return Err("Actual recovery source changed".into());
            }
            let completed = &old
                .start
                .as_ref()
                .ok_or("Actual original recovery projection missing")?
                .projection;
            let JobOutcome::Success(value) = &completed.outcome else {
                return Err("Actual original registered projection failed".into());
            };
            let mut input = Some(ProjectionInput::restart(
                resources.client.clone(),
                root.capture_root.clone(),
                sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                    &root.working_directory,
                )
                .user(),
                super::super::projection::RestartProjection {
                    source_custody: value.source.clone(),
                    source: value.registered.clone(),
                    previous: self.previous,
                    run: self.identity.run,
                    original: self.original,
                    connection: value.entry_connection,
                    request: value.entry_request,
                },
                resources.authority.running_installation(),
            ));
            let spec = JobSpec {
                id: resources.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(self.original.until()),
                reservation_bytes: super::super::projection::PROJECTION_BYTES,
                replacement: None,
            };
            match resources.client.submit_blocking_owned_box(
                resources.finite,
                spec,
                &mut input,
                super::super::projection::project,
            ) {
                Ok(job) => self.job = Some(job),
                Err(OwnedInputError::Admission(
                    AdmissionError::QueueSaturated { .. } | AdmissionError::MemorySaturated { .. },
                )) => return Ok(()),
                Err(cause) => return Err(cause.into()),
            }
            return Ok(());
        }
        if self.grant.is_none() {
            let completed = self.completion.as_mut().ok_or("Actual recovery completion missing")?;
            let JobOutcome::Success(value) = &mut completed.outcome else {
                return Err(
                    "Actual recovery projection failed with original completion retained".into()
                );
            };
            let snapshot =
                value.snapshot.take().ok_or("Actual once-only recovery snapshot missing")?;
            let source = value.registered.clone();
            let next = root
                .next_recovery_grant
                .checked_add(1)
                .ok_or("Recovery grant identity exhausted")?;
            let key = RecoveryGrantKey::new(
                RecoveryGrantId::new(next)?,
                self.previous,
                self.selected.binding(),
            )?;
            match resources
                .client
                .issue_registered_recovery_grant(snapshot, &source, completed, key)
            {
                Ok(grant) => {
                    self.grant = Some(grant);
                    root.next_recovery_grant = next;
                }
                Err(rejected) => {
                    if let JobOutcome::Success(value) = &mut completed.outcome {
                        value.snapshot = Some(rejected.snapshot);
                    }
                    return Err(rejected.error.into());
                }
            }
        }
        let completed =
            self.completion.as_mut().ok_or("Actual successful recovery completion missing")?;
        if let JobOutcome::Success(value) = &mut completed.outcome
            && value.policy_credit.is_some()
        {
            root.backend
                .as_mut()
                .ok_or("Actual recovery policy owner missing")?
                .add_projection_credit(&mut value.policy_credit)
                .map_err(outer)?;
        }
        let storage = completed.memory.claim(size_of::<Record>() as u64)?;
        let completed =
            self.completion.take().ok_or("Actual recovery projection transfer missing")?;
        let grant = self.grant.as_ref().ok_or("Actual recovery grant missing")?.clone();
        let start = match DesktopStart::new_recovery(
            self.identity,
            Default::default(),
            self.connection,
            completed,
            grant,
            resources,
        ) {
            Ok(start) => start,
            Err(rejected) => {
                let (cause, completed, grant) = rejected.into_parts();
                self.completion = Some(completed);
                self.grant = Some(grant);
                return Err(cause);
            }
        };
        let old = root.records[self.at].as_ref().ok_or("Actual recovery source disappeared")?;
        let mut record = Record::new(
            self.identity,
            self.connection,
            Default::default(),
            old.record.replied,
            false,
        );
        record.start = Some(start);
        self.record = Some(RetainedRecord { record: Box::new(record), _storage: storage });
        Ok(())
    }
}
