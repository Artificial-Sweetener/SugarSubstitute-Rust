// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite launcher operations with role-specific reservations and original cutoffs.
use super::*;

impl Launcher {
    /// Retain exactly one worker under startup or independent cleanup scope.
    pub(super) fn submit(
        &mut self,
        task: Task,
        body: impl FnOnce(JobContext) -> io::Result<Work> + Send + 'static,
    ) -> Result<()> {
        let cleanup = matches!(task, Task::Retire | Task::Recover | Task::StopEngine);
        if self.job.is_some() || (self.closing && !matches!(task, Task::Retire | Task::StopEngine))
        {
            return Err("Launcher admission is closed or occupied".into());
        }
        let client = self.client.as_ref().ok_or("Launcher execution absent")?;
        let scope = if cleanup { self.cleanup_scope.as_ref() } else { self.work_scope.as_ref() }
            .ok_or("Launcher scope absent")?;
        let deadline = if matches!(task, Task::Recover)
            || (matches!(task, Task::StopEngine) && !self.closing)
        {
            self.recovery_until.ok_or("Launcher trial recovery cutoff absent")?
        } else if cleanup {
            self.cleanup_until.ok_or("Launcher cleanup cutoff absent")?
        } else {
            self.until.ok_or("Launcher cutoff absent")?
        };
        if client.now()? >= deadline {
            return Err("Launcher admission reached its original cutoff".into());
        }
        let job = client.submit_blocking(
            scope,
            JobSpec {
                id: self.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(deadline),
                reservation_bytes: match task {
                    Task::Cache => 1024 * 1024,
                    Task::OpenEngine => setup_storage::TRUST_RESERVATION_BYTES,
                    Task::VerifyEngine => {
                        setup_storage::RESERVATION_BYTES
                            + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
                    }
                    Task::PrepareEngine | Task::ObserveEngine | Task::StopEngine => {
                        sugarsubstitute_supervision::application_launch::ACTIVATION_RESERVATION_BYTES
                    }
                    Task::CreateEngine | Task::ObserveOffer => {
                        sugarsubstitute_supervision::application_launch::CREATION_RESERVATION_BYTES
                    }
                    Task::Trial => setup_storage::SIGNED_TRANSFER_RESERVATION_BYTES,
                    Task::Create { .. } if self.source.is_some() => {
                        sugarsubstitute_supervision::application_launch::ACTIVATION_RESERVATION_BYTES
                    }
                    Task::Create { .. } => {
                        sugarsubstitute_supervision::application_launch::CREATION_RESERVATION_BYTES
                    }
                    Task::Recover | Task::Retire => {
                        sugarsubstitute_supervision::application_launch::RECOVERY_RESERVATION_BYTES
                    }
                    Task::Installation | Task::Select => setup_storage::LAUNCH_RESERVATION_BYTES,
                },
                replacement: None,
            },
            body,
        )?;
        self.job = Some(Pending { task, job });
        Ok(())
    }
}
