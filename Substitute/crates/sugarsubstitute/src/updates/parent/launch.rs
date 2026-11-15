// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the installed executable before admitting its contained preparation role.
use super::*;
use std::sync::{Mutex, PoisonError};
use sugarsubstitute_contracts::{
    execution::AdmissionError,
    update::attempt::codec::{UpdateAttemptChannel, UpdateAttemptKey},
};
use sugarsubstitute_execution::OwnedInputError;
use sugarsubstitute_supervision::UpdateAttemptPins;

/// The attempt channel's pins, kept by this owner until the job that composes the helper is
/// admitted and takes them.
pub(super) type Pins = Arc<Mutex<Option<UpdateAttemptPins>>>;

/// Submit the job that composes the helper from `source`. Returns `None`, with `source` still
/// in place, while another startup job holds the memory this one reserves: the caller submits
/// it again once that job finishes, which wakes it, and the check's limit still bounds the
/// wait. Every other refusal is the caller's failure.
pub(super) fn compose<I, T, M>(
    client: &ExecutionClient,
    work: &Scope,
    spec: JobSpec,
    source: &mut Option<I>,
    factory: M,
) -> Result<Option<OwnedJob<T, io::Error>>>
where
    I: Send + 'static,
    T: Send + 'static,
    M: FnOnce(I, sugarsubstitute_contracts::execution::JobContext) -> io::Result<T>
        + Send
        + 'static,
{
    match client.submit_blocking_owned_input(work, spec, source, factory) {
        Ok(job) => Ok(Some(job)),
        Err(OwnedInputError::Admission(AdmissionError::MemorySaturated { .. })) => Ok(None),
        Err(cause) => Err(cause.into()),
    }
}

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
            let native = self.channel.take().ok_or("Update attempt scope absent")?;
            self.attempt = Some(AttemptChannel::reserve(
                &self.client,
                &self.cleanup,
                native,
                spec,
                channel,
                handshake,
            )?);
        }
        // The helper is composed only once its channel's endpoint exists.
        let pins: Option<Pins> = if self.preparing.is_none()
            && self.retained.as_ref().is_some_and(|held| held.source.is_some())
        {
            self.composing.take().or_else(|| {
                self.attempt.as_mut().and_then(AttemptChannel::take_pins).map(
                    |(bootstrap, enrollment)| {
                        Arc::new(Mutex::new(Some(UpdateAttemptPins { bootstrap, enrollment })))
                    },
                )
            })
        } else {
            None
        };
        if let Some(pins) = pins {
            let handed = Arc::clone(&pins);
            let key = self.key;
            let spec = self.spec(
                jobs,
                JobClass::InfrastructureLifecycle,
                setup_storage::LAUNCH_RESERVATION_BYTES,
                Some(self.step_deadline()?),
            )?;
            let source = &mut self.retained.as_mut().ok_or("Update custody absent")?.source;
            let composed = compose(
                &self.client,
                &self.work,
                spec,
                source,
                move |application: Arc<RunningApplication>, context| {
                    let pins = handed
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take()
                        .ok_or_else(|| io::Error::other("Update attempt pins already taken"))?;
                    let image = application.pin_preparation(&context)?;
                    let workspace = image.installation().join(".update-workspace");
                    let environment =
                        crate::worker_environment::capture().map_err(io::Error::other)?;
                    WindowsLaunchInput::preparation(key, image, &workspace, environment, Some(pins))
                },
            )?;
            match composed {
                Some(job) => self.preparing = Some(job),
                None => {
                    self.composing = Some(pins);
                    return Ok(());
                }
            }
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
