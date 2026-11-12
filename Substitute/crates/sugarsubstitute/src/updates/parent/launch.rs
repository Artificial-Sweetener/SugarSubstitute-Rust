// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the installed executable before admitting its contained preparation role.
use super::*;
use sugarsubstitute_contracts::update::attempt::codec::{UpdateAttemptChannel, UpdateAttemptKey};
impl Parent {
    /// Each phase retains actual partial admission; closing never starts a later phase.
    pub(super) fn launch(&mut self, jobs: &JobSequence) -> Result<()> {
        let held = self.retained.as_ref().ok_or("Update custody absent")?;
        if held.registry.is_none() && self.constructing.is_none() {
            let spec = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                WindowsProcessRegistry::required_reservation_bytes(),
                Some(self.step_deadline()?),
            )?;
            self.constructing =
                Some(WindowsProcessRegistry::submit_new(&self.client, &self.work, spec, self.key)?);
        }
        if self.attempt.is_none()
            && !self.probe
            && self.retained.as_ref().is_some_and(|held| held.source.is_some())
        {
            let spec = self.spec(
                jobs,
                JobClass::NativeObservation,
                sugarsubstitute_execution::purpose_pipe::MIN_LIFECYCLE_PIPE_BYTES,
                None,
            )?;
            let attempt = std::num::NonZeroU64::new(spec.id.get()).ok_or("Attempt id absent")?;
            let channel = UpdateAttemptChannel::new(self.key, UpdateAttemptKey::new(attempt));
            let handshake = self.client.now()?.checked_add(super::attempt::HANDSHAKE)?;
            // The channel lives in the cleanup scope, so cancelling work never loses the result.
            self.attempt = Some(AttemptChannel::reserve(
                &self.client,
                &self.cleanup,
                spec,
                channel,
                handshake,
            )?);
        }
        // The helper is composed only once its channel's endpoint exists.
        let pins = if self.preparing.is_none()
            && self.retained.as_ref().is_some_and(|held| held.source.is_some())
        {
            self.attempt.as_mut().and_then(AttemptChannel::take_pins)
        } else {
            None
        };
        if let Some((bootstrap, enrollment)) = pins {
            let pins = sugarsubstitute_supervision::UpdateAttemptPins { bootstrap, enrollment };
            let key = self.key;
            let spec = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                setup_storage::LAUNCH_RESERVATION_BYTES,
                Some(self.step_deadline()?),
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
                    WindowsLaunchInput::preparation(key, image, &workspace, environment, Some(pins))
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
                Some(self.step_deadline()?),
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
            if let Some(channel) = &mut self.attempt {
                channel.bind(ticket.process())?;
            }
        }
        Ok(())
    }
}
