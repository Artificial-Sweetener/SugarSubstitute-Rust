// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the installed executable before admitting its contained preparation role.
use super::*;
impl Parent {
    /// Each phase retains actual partial admission; closing never starts a later phase.
    pub(super) fn launch(&mut self, jobs: &JobSequence) -> Result<()> {
        let held = self.retained.as_ref().ok_or("Update custody absent")?;
        if held.registry.is_none() && self.constructing.is_none() {
            let spec = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                WindowsProcessRegistry::required_reservation_bytes(),
                Some(self.until),
            )?;
            self.constructing =
                Some(WindowsProcessRegistry::submit_new(&self.client, &self.work, spec, self.key)?);
        }
        if self.preparing.is_none()
            && self.retained.as_ref().is_some_and(|held| held.source.is_some())
        {
            let key = self.key;
            let spec = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                setup_storage::LAUNCH_RESERVATION_BYTES,
                Some(self.until),
            )?;
            let source = &mut self.retained.as_mut().ok_or("Update custody absent")?.source;
            self.preparing = Some(self.client.submit_blocking_owned_input(
                &self.work,
                spec,
                source,
                move |application, context| {
                    let image = application.pin_preparation(&context)?;
                    let workspace = image.installation().join(".update-workspace");
                    let environment =
                        crate::worker_environment::capture().map_err(io::Error::other)?;
                    WindowsLaunchInput::preparation(key, image, &workspace, environment)
                },
            )?);
        }
        let held = self.retained.as_ref().ok_or("Update custody absent")?;
        if self.process.is_none()
            && held.registry.is_some()
            && let Some(input) = &held.input
        {
            let watch = self.spec(jobs, JobClass::NativeObservation, 4096, None)?;
            let create = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                input.required_reservation_bytes(),
                Some(self.until),
            )?;
            let held = self.retained.as_mut().ok_or("Update custody absent")?;
            let ticket =
                held.registry.as_mut().ok_or("Update registry absent")?.submit_preparation_launch(
                    &self.client,
                    &self.work,
                    self.native,
                    create,
                    watch,
                    &mut held.input,
                )?;
            self.process = Some(ticket.process());
        }
        Ok(())
    }
}
