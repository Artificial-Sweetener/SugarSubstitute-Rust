// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain provisioning containment until the actual registry reports complete native retirement.
use super::*;
impl Worker {
    pub(super) fn launch_input(&mut self, input: WindowsLaunchInput) -> Result<()> {
        let metadata = input.provisioning_metadata()?;
        let watch = self.spec(JobClass::NativeObservation, 4096, None)?;
        let creation = self.spec(
            JobClass::InfrastructureLifecycle,
            input.required_reservation_bytes(),
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        let binding = FamilyBinding::Preparation(self.identity);
        let ticket = self.registry.as_mut().ok_or("Provisioning registry missing")?.submit_launch(
            &self.client,
            &self.scope,
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            creation,
            watch,
            binding,
            input,
        )?;
        self.process = Some(ticket.process());
        self.metadata_access = Some(metadata);
        Ok(())
    }
    pub(super) fn cleanup(&mut self) -> Result<()> {
        let Some(process) = self.process else {
            return self.finalize_cache();
        };
        let now = self.client.now()?;
        if !self.stopped {
            self.registry.as_mut().ok_or("Provisioning registry missing")?.begin_stop(
                process,
                ShutdownId::new(1)?,
                ShutdownBudget::new(now)?,
            )?;
            self.stopped = true;
        }
        let snapshot =
            self.registry.as_ref().ok_or("Provisioning registry missing")?.snapshot(process)?;
        if snapshot.settled {
            match self
                .registry
                .as_mut()
                .ok_or("Provisioning registry missing")?
                .take_settled_provisioning(process)
            {
                Ok(resources) => self.retired = Some(resources),
                Err(sugarsubstitute_supervision::RegistryAdmissionError::Busy) => return Ok(()),
                Err(sugarsubstitute_supervision::RegistryAdmissionError::Closed) => {}
                Err(cause) => return Err(cause.into()),
            }
            self.registry
                .as_mut()
                .ok_or("Provisioning registry missing")?
                .retire_settled(process)?;
            self.process = None;
            self.finalize_cache()?;
        } else if snapshot.cleanup_step_due(now) {
            let watch = if snapshot.member_watch_required {
                Some(self.spec(JobClass::NativeObservation, 4096, None)?)
            } else {
                None
            };
            let cleanup = self.spec(JobClass::InfrastructureLifecycle, 4096, None)?;
            self.registry.as_mut().ok_or("Provisioning registry missing")?.submit_cleanup_step(
                &self.client,
                process,
                &self.scope,
                cleanup,
                watch,
            )?;
        } else if snapshot.deadline_expired(now) {
            return Err("Provisioning family remains unsettled after cleanup deadline".into());
        }
        Ok(())
    }
}
