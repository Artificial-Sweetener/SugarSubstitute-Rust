// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe activation handoff independently of ordinary application Root creation.
mod exchange;
mod native;
#[cfg(test)]
mod tests;
use super::*;
use crate::setup_storage::PreparationImage;
use std::os::windows::io::AsRawHandle;
use sugarsubstitute_contracts::application_launch::updater::ACTIVATION_ARGUMENT;
use sugarsubstitute_contracts::application_launch::updater::activation::ActivationRequest;
use sugarsubstitute_execution::activation_channel::{CHANNEL_RESERVATION_BYTES, PreparedChannel};
use windows::Win32::Foundation::HANDLE;

/// Channel, decoded transfer and native creation admission for one finite activation worker.
pub const ACTIVATION_RESERVATION_BYTES: u64 = CHANNEL_RESERVATION_BYTES
    + super::broker::TRANSFER_RESERVATION_BYTES
    + CREATION_RESERVATION_BYTES;

/// Observed activation disposition. Terminal outcomes require native output retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationOutcome {
    /// The engine or its contained descendants still require observation.
    Pending,
    /// Owned termination is requested, but actual membership has not reached zero.
    Settling {
        /// Actual root status when available; this alone never authorizes fallback.
        exit_code: Option<u32>,
    },
    /// No engine process remains and creator/channel outputs have retired.
    Stopped {
        /// Actual engine exit status, when a process was created and observed.
        exit_code: Option<u32>,
        /// Actual independent creation fact, never inferred from the engine exit status.
        creation: CreationOutcome,
    },
}

/// Retain the exact engine, native containment and process handles outside execution results.
/// Its owner must poll through transfer or settlement. Owner loss closes a kill-on-close Job;
/// that containment action does not claim settlement or permit an automatic fallback.
#[derive(Clone)]
pub struct ActivationLaunch(Arc<Mutex<Activation>>);
/// Keep authenticated files and native outputs together through one irreversible disposition.
struct Activation {
    image: Option<Box<PreparationImage>>,
    command: Option<WindowsLaunchBuffers>,
    issued: bool,
    resumed: bool,
    decision: Decision,
    exit_code: Option<u32>,
    job: Option<Handle>,
    process: Option<Handle>,
    thread: Option<Handle>,
    memory: Option<Arc<MemoryClaim>>,
    exchange: Option<exchange::Exchange>,
    creation: CreationOutcome,
    original: Option<JobContext>,
    cleanup: Option<JobContext>,
}
/// Native transfer or settlement remains authoritative after result delivery fails.
#[derive(Clone, Copy)]
enum Decision {
    Active,
    Stopping,
    Stopped,
}
impl Drop for Activation {
    fn drop(&mut self) {
        // Owner-loss containment precedes releasing image pins. Only explicit polling proves
        // settlement. The independent application is never a member of this Job.
        drop(self.job.take());
    }
}

impl ActivationLaunch {
    /// Borrow only the actual authenticated, resumed engine while its ownership lock remains held.
    /// A broker can duplicate source objects without PID lookup or trusting handle numbers alone.
    pub(super) fn with_process<T>(
        &self,
        body: impl FnOnce(HANDLE) -> Result<T, ClaimError>,
    ) -> Result<T, ClaimError> {
        let state = crate::native_claims::lock(&self.0);
        if !state.resumed || !matches!(state.decision, Decision::Active) || state.image.is_none() {
            return Err(ClaimError::invalid("activation_transfer_source_closed"));
        }
        body(
            state
                .process
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("activation_process_absent"))?
                .raw(),
        )
    }

    /// Adopt pinned accepted files without native work. Retain this owner through terminal observation.
    pub fn new(image: Box<PreparationImage>) -> Self {
        Self(Arc::new(Mutex::new(Activation {
            image: Some(image),
            command: None,
            issued: false,
            resumed: false,
            decision: Decision::Active,
            exit_code: None,
            job: None,
            process: None,
            thread: None,
            memory: None,
            exchange: None,
            creation: CreationOutcome::NotCreated,
            original: None,
            cleanup: None,
        })))
    }

    /// Compose the authenticated activation entry on an admitted worker. The engine independently
    /// authenticates its running image and cached candidate; only the channel is inherited.
    /// # Errors
    /// Rejects closed/repeated admission, unsupported capabilities, cancellation and invalid paths.
    pub fn prepare(&self, context: &JobContext) -> Result<(), ClaimError> {
        check(context)?;
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued
            || state.command.is_some()
            || state.exchange.is_some()
            || state.original.is_some()
        {
            return Err(ClaimError::invalid("activation_preparation_closed"));
        }
        let image =
            state.image.as_ref().ok_or_else(|| ClaimError::invalid("activation_image_absent"))?;
        image.require_activation().map_err(|cause| ClaimError::new("activation_entry", cause))?;
        let executable = image.executable();
        let directory = image.directory().to_owned();
        let installation = image.installation().as_os_str().to_owned();
        state.original = Some(context.clone());
        let channel = PreparedChannel::prepare(context).map_err(exchange::channel_error)?;
        state.exchange = Some(exchange::Exchange::new(channel)?);
        let exchange = state
            .exchange
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("activation_exchange_absent"))?;
        let encoded = exchange
            .channel
            .with_child(|handle| {
                Ok(ActivationRequest::new(
                    handle.as_raw_handle() as usize as u64,
                    exchange.channel.identity(),
                ))
            })
            .map_err(exchange::channel_error)?
            .map_err(|cause| ClaimError::new("activation_request", cause))?
            .encode()
            .map_err(|cause| ClaimError::new("activation_request", cause))?;
        state.command = Some(
            WindowsLaunchCommand::from_os(
                &executable,
                &directory,
                &[ACTIVATION_ARGUMENT.into(), installation, encoded.into()],
            )
            .map_err(|cause| ClaimError::new("activation_command", cause))?
            .into_native_parts(),
        );
        Ok(())
    }

    /// Create once in a private kill-on-close Job, adopt outputs, then resume the engine.
    /// Run on an admitted worker with `CREATION_RESERVATION_BYTES`. Successful return means
    /// creation only; keep polling until transfer or actual settlement is observed.
    /// # Errors
    /// Preserves native creation/resume/close failures and every produced output for `poll(..., true)`.
    pub fn launch(&self, context: &JobContext) -> Result<(), ClaimError> {
        self.launch_with(context, super::creation::contained, native::resume)
    }

    /// Production supplies exact native creation/resume boundaries; tests retain real containment.
    fn launch_with(
        &self,
        context: &JobContext,
        create: impl FnOnce(
            &mut WindowsLaunchBuffers,
            HANDLE,
            usize,
            &mut PROCESS_INFORMATION,
        ) -> Result<(), ClaimError>,
        resume: impl FnOnce(HANDLE) -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        check(context)?;
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued || state.command.is_none() {
            return Err(ClaimError::invalid("activation_creation_closed"));
        }
        state.admit(context, false)?;
        state.issued = true;
        let memory = Arc::new(
            context
                .memory()
                .claim(262144)
                .map_err(|cause| ClaimError::new("activation_memory", cause))?,
        );
        let budget = Budget::Protected(memory.clone());
        state.memory = Some(memory);
        state.job = Some(native::job(budget.clone())?);
        let job =
            state.job.as_ref().ok_or_else(|| ClaimError::invalid("activation_job_absent"))?.raw();
        let mut output = PROCESS_INFORMATION::default();
        let Activation { command, exchange, .. } = &mut *state;
        let command =
            command.as_mut().ok_or_else(|| ClaimError::invalid("activation_command_absent"))?;
        let created = exchange
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("activation_exchange_absent"))?
            .channel
            .with_child(|handle| {
                Ok(create(command, job, handle.as_raw_handle() as usize, &mut output))
            })
            .map_err(exchange::channel_error)?;
        if !output.hProcess.is_invalid() {
            state.process = Some(Handle::new(output.hProcess, budget.clone()));
        }
        if !output.hThread.is_invalid() {
            state.thread = Some(Handle::new(output.hThread, budget));
        }
        created?;
        state
            .exchange
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("activation_exchange_absent"))?
            .channel
            .retire_child()
            .map_err(exchange::channel_error)?;
        check(context)?;
        if state.process.is_none() {
            return Err(ClaimError::invalid("activation_process_absent"));
        }
        let thread =
            state.thread.as_mut().ok_or_else(|| ClaimError::invalid("activation_thread_absent"))?;
        resume(thread.raw())?;
        state.resumed = true;
        if let Some(thread) = &mut state.thread {
            thread.close()?;
        }
        state.thread = None;
        tracing::info!(
            operation = "activation_engine_created",
            pid = output.dwProcessId,
            "Activation engine is contained and observed"
        );
        Ok(())
    }

    /// Observe once without waiting, on an admitted native worker. `stop` closes creation and
    /// requests termination of only this owned Job; fallback requires `Stopped`, never a timeout
    /// or a missing lock. Preserve this owner and repeat on activity while `Pending`.
    /// The independent application's creation fact survives cancellation and discarded results.
    /// # Errors
    /// Preserves query/termination/close failures and still-owned handles for bounded cleanup.
    pub fn poll(&self, context: &JobContext, stop: bool) -> Result<ActivationOutcome, ClaimError> {
        check(context)?;
        let mut state = crate::native_claims::lock(&self.0);
        state.admit(context, stop)?;
        if !state.issued && !stop {
            return Err(ClaimError::invalid("activation_not_started"));
        }
        state.issued = true;
        state.observe(stop)?;
        if matches!(state.decision, Decision::Active) && state.resumed {
            let process = state
                .process
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("activation_process_absent"))?
                .raw();
            state
                .exchange
                .as_mut()
                .ok_or_else(|| ClaimError::invalid("activation_exchange_absent"))?
                .advance(process, context)?;
        }
        let outcome = match state.decision {
            Decision::Active => return Ok(ActivationOutcome::Pending),
            Decision::Stopping => {
                return Ok(ActivationOutcome::Settling { exit_code: state.exit_code });
            }
            Decision::Stopped => {
                let creation = if let Some(exchange) = &mut state.exchange {
                    if !exchange.retire(context)? {
                        return Ok(ActivationOutcome::Settling { exit_code: state.exit_code });
                    }
                    exchange.outcome()?
                } else {
                    state.creation
                };
                state.creation = creation;
                ActivationOutcome::Stopped { exit_code: state.exit_code, creation }
            }
        };
        state.retire()?;
        Ok(outcome)
    }
}

impl Activation {
    /// Later worker scopes cannot renew ordinary admission or its first cleanup cutoff.
    fn admit(&mut self, context: &JobContext, cleanup: bool) -> Result<(), ClaimError> {
        if self.original.as_ref().is_some_and(|original| original.owner_id() != context.owner_id())
        {
            return Err(ClaimError::invalid("activation_execution_owner"));
        }
        let original = if cleanup {
            self.cleanup.get_or_insert_with(|| context.clone())
        } else {
            if self.cleanup.is_some() {
                return Err(ClaimError::invalid("activation_admission_closed"));
            }
            self.original.get_or_insert_with(|| context.clone())
        };
        check(original)
    }
    /// Commit actual native facts before any fallible handle retirement or result publication.
    fn observe(&mut self, stop: bool) -> Result<(), ClaimError> {
        if matches!(self.decision, Decision::Stopped) {
            return Ok(());
        }
        let Some(job) = &self.job else {
            if self.process.is_some() {
                return Err(ClaimError::invalid("activation_containment_lost"));
            }
            self.decision = Decision::Stopped;
            return Ok(());
        };
        if let Some(process) = &self.process {
            self.exit_code = native::exit(process.raw())?;
        }
        if matches!(self.decision, Decision::Active) && (stop || self.exit_code.is_some()) {
            native::terminate(job.raw())?;
            self.decision = Decision::Stopping;
        }
        if matches!(self.decision, Decision::Stopping) && native::empty(job.raw())? {
            self.decision = Decision::Stopped;
        }
        Ok(())
    }
    /// Release file pins only after all native outputs have closed under the committed decision.
    fn retire(&mut self) -> Result<(), ClaimError> {
        for slot in [&mut self.thread, &mut self.process, &mut self.job] {
            if let Some(handle) = slot {
                handle.close()?;
            }
            *slot = None;
        }
        self.command = None;
        self.image = None;
        self.memory = None;
        self.exchange = None;
        Ok(())
    }
}

/// Admission and observation retain their caller's original deadline and cancellation scope.
fn check(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err(ClaimError::invalid("activation_worker_admission"));
    }
    Ok(())
}
