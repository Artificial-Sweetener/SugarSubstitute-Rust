// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer startup to a fresh, independently owned application Root.
pub mod broker;
mod creation;
#[cfg(test)]
pub(crate) mod rejected_creation;
#[cfg(test)]
mod tests;
mod trial;
mod updater;
use crate::{
    ClaimError, WindowsLaunchBuffers, WindowsLaunchCommand,
    native_claims::files::{self, Budget, Handle},
};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
pub use updater::{ACTIVATION_RESERVATION_BYTES, ActivationLaunch, ActivationOutcome};
use windows::Win32::System::Threading::PROCESS_INFORMATION;

/// Native creation of an already prepared command needs bounded handles and attributes only.
/// Reserve this alongside retained selection/trial custody; payload verification has its own job.
pub const CREATION_RESERVATION_BYTES: u64 = 1024 * 1024;
/// Recovery uses an existing pinned store to restore an unstarted selection and close creator handles.
/// Reserve this alongside retained trial custody; it does not reopen or verify application payloads.
pub const RECOVERY_RESERVATION_BYTES: u64 =
    crate::setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES;

/// Actual native creation fact retained independently of job completion and handle retirement.
/// Creation does not establish startup health or guarantee that the process is still running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreationOutcome {
    /// Native creation produced no process; an exact unstarted trial can be restored.
    NotCreated,
    /// Windows confirmed creation. Never restore this attempt or automatically repeat it.
    Created,
    /// Partial native output or an unanswered launch transfer leaves creation uncertain.
    /// Process absence has not been proved, so automatic fallback is prohibited.
    Indeterminate,
}
impl CreationOutcome {
    /// Classify native status before adopting outputs or attempting any handle cleanup.
    fn observed(success: bool, process: &PROCESS_INFORMATION) -> Self {
        if success {
            Self::Created
        } else if !process.hProcess.is_invalid() || !process.hThread.is_invalid() {
            Self::Indeterminate
        } else {
            Self::NotCreated
        }
    }
}

/// Keep creation outputs owned even if the admitted job's result is cancelled or discarded.
/// The new Root owns its own lifetime from successful creation; this owner never terminates it.
#[derive(Clone)]
pub struct ApplicationLaunch(Arc<Mutex<State>>);
/// Actual verified input retained through creation and native output retirement.
enum LaunchCustody {
    Selected(Box<crate::setup_storage::LaunchInheritance>),
    Trial(Box<trial::TrialLaunch>),
    Broker(Box<broker::CapturedPins>),
}
struct State {
    command: Option<WindowsLaunchBuffers>,
    issued: bool,
    creation: CreationOutcome,
    trial_restored: bool,
    handles: Vec<Handle>,
    memory: Option<Arc<MemoryClaim>>,
    custody: Option<LaunchCustody>,
    transfer: Option<broker::source::Transfer>,
}
impl Drop for State {
    fn drop(&mut self) {
        if self.transfer.as_ref().is_some_and(broker::source::Transfer::pending) {
            // A live receiver may still duplicate these source-process handle numbers. Keep
            // their original objects until this dedicated engine exits; never permit slot reuse.
            tracing::error!(
                operation = "launch_transfer_unanswered_retained",
                "Unanswered launch custody remains retained until engine exit"
            );
            std::mem::forget(self.custody.take());
            std::mem::forget(self.memory.take());
        }
    }
}
impl ApplicationLaunch {
    /// Retain inert, already bounded executable/cwd/argv buffers. Performs no native work.
    pub fn new(command: WindowsLaunchCommand) -> Self {
        let launch = Self::empty();
        crate::native_claims::lock(&launch.0).command = Some(command.into_native_parts());
        launch
    }
    /// Allocate creator state without doing native work or admitting a process.
    fn empty() -> Self {
        Self(Arc::new(Mutex::new(State {
            command: None,
            issued: false,
            creation: CreationOutcome::NotCreated,
            trial_restored: false,
            handles: Vec::new(),
            memory: None,
            custody: None,
            transfer: None,
        })))
    }
    /// Keep verified installation custody until the exact inherited-handle launch is created.
    /// # Errors
    /// Rejects invalid native command encoding while preserving the original source cause.
    pub fn selected(
        selection: crate::setup_storage::LaunchInheritance,
    ) -> Result<Self, ClaimError> {
        let command = WindowsLaunchCommand::from_os(
            &selection.application().executable(),
            selection.application().directory(),
            &selection.arguments(),
        )
        .map_err(|cause| ClaimError::new("application_launch_command", cause))?;
        let launch = Self::new(command);
        crate::native_claims::lock(&launch.0).custody =
            Some(LaunchCustody::Selected(Box::new(selection)));
        Ok(launch)
    }
    /// Create the replacement exactly once in an admitted blocking job, then close creator handles.
    /// Reserve `CREATION_RESERVATION_BYTES`. Replacement callers retire original application/conflict custody first. Stable launchers
    /// retain only selection custody; an existing coordinator handles repeated-launch forwarding.
    /// # Errors
    /// Returns original creation/cleanup failures. Trial callers use `recover_and_retire` on
    /// their independent cleanup worker; other launches use `retire` for partial outputs.
    pub fn launch(&self, context: &JobContext) -> Result<(), ClaimError> {
        self.launch_with(context, creation::create)
    }
    /// Keep native status separate from every actual output at the process-creation boundary.
    fn launch_with(
        &self,
        context: &JobContext,
        create: impl FnOnce(
            &mut WindowsLaunchBuffers,
            Option<&LaunchCustody>,
            &mut PROCESS_INFORMATION,
        ) -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued
            || context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err(ClaimError::invalid("application_launch_admission"));
        }
        state.memory = Some(Arc::new(
            context
                .memory()
                .claim(262144)
                .map_err(|cause| ClaimError::new("application_launch_memory", cause))?,
        ));
        state
            .handles
            .try_reserve_exact(2)
            .map_err(|cause| ClaimError::new("application_launch_handles", cause))?;
        let budget = Budget::Protected(
            state
                .memory
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("application_launch_memory"))?
                .clone(),
        );
        state.issued = true;
        let mut process = PROCESS_INFORMATION::default();
        let State { command, custody, .. } = &mut *state;
        let command =
            command.as_mut().ok_or_else(|| ClaimError::invalid("application_launch_command"))?;
        let created = create(command, custody.as_ref(), &mut process);
        state.creation = CreationOutcome::observed(created.is_ok(), &process);
        if !process.hProcess.is_invalid() {
            state.handles.push(Handle::new(process.hProcess, budget.clone()));
        }
        if !process.hThread.is_invalid() {
            state.handles.push(Handle::new(process.hThread, budget));
        }
        created?;
        tracing::info!(
            operation = "application_launch_created",
            pid = process.dwProcessId,
            "Transferred startup to a fresh application Root"
        );
        state.retire()
    }
    /// Close only this creator's retained handles; never stop the independent replacement.
    /// # Errors
    /// Preserves native close failures and their still-owned handles for another cleanup attempt.
    /// An unstarted trial requires `recover_and_retire` before its custody can be released.
    pub fn retire(&self) -> Result<(), ClaimError> {
        crate::native_claims::lock(&self.0).retire()
    }

    /// Close creation admission and restore an unstarted trial before retiring creator custody.
    /// Reserve `RECOVERY_RESERVATION_BYTES` on an independent admitted cleanup worker after observing the creation job's terminal
    /// outcome. Any successful native creation or partial native output excludes restoration.
    /// Returns the retained native creation fact even if the original worker result was discarded.
    /// This never stops a created application or infers descendant settlement from a missing lock.
    /// # Errors
    /// Preserves cancellation, stale selection, publication and native close failures. Retain this
    /// owner and retry cleanup on failure; a restored record is not published twice.
    pub fn recover_and_retire(&self, context: &JobContext) -> Result<CreationOutcome, ClaimError> {
        let mut state = crate::native_claims::lock(&self.0);
        state.issued = true;
        if state.creation == CreationOutcome::NotCreated
            && !state.trial_restored
            && let Some(LaunchCustody::Trial(trial)) = &state.custody
        {
            trial.restore(context)?;
            state.trial_restored = true;
        }
        state.retire()?;
        Ok(state.creation)
    }
}
impl State {
    fn retire(&mut self) -> Result<(), ClaimError> {
        if self.transfer.as_ref().is_some_and(broker::source::Transfer::pending) {
            return Err(ClaimError::invalid("launch_transfer_receiver_unsettled"));
        }
        if self.creation == CreationOutcome::NotCreated
            && !self.trial_restored
            && matches!(self.custody, Some(LaunchCustody::Trial(_)))
        {
            return Err(ClaimError::invalid("application_launch_trial_requires_recovery"));
        }
        files::close_all(&mut self.handles)?;
        if let Some(LaunchCustody::Broker(pins)) = &mut self.custody {
            pins.retire()?;
        }
        self.memory = None;
        self.command = None;
        self.custody = None;
        if let Some(transfer) = &mut self.transfer {
            transfer.closed = true;
        }
        Ok(())
    }
}
