// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The stable creator duplicates exact engine custody and retains actual native creation outcomes.
mod native;
use super::*;
use windows::Win32::Foundation::HANDLE;

/// Native file and entry duplicates retained by the ordinary application creation owner.
pub(in crate::application_launch) struct CapturedPins {
    handles: Vec<Handle>,
    inherited: [usize; 2],
    count: usize,
    _memory: Arc<MemoryClaim>,
}
impl CapturedPins {
    /// Include only remapped entry handles in the child's explicit inheritance allowlist.
    pub(in crate::application_launch) fn inheritance(&self) -> ([usize; 2], usize) {
        (self.inherited, self.count)
    }
    /// Preserve failed native closes for retry before releasing any retained memory allowance.
    pub(in crate::application_launch) fn retire(&mut self) -> Result<(), ClaimError> {
        files::close_all(&mut self.handles)
    }
}

/// One request's receiver custody survives rejected, cancelled and discarded worker completions.
/// Capture, creation and retirement run on admitted workers, in order. This owner starts the
/// application in its own process context; it never puts it inside the update engine's Job.
#[derive(Clone)]
pub struct LaunchReceiver(Arc<Mutex<Receiver>>);
/// Every native output is adopted here before checking the operation that produced it.
struct Receiver {
    id: [u8; 32],
    issued: bool,
    handles: Vec<Handle>,
    memory: Option<Arc<MemoryClaim>>,
    application: Option<ApplicationLaunch>,
    outcome: Option<CreationOutcome>,
    original: Option<JobContext>,
}

/// Correlated actual creation result, encoded only after creator outputs have retired.
pub struct LaunchResult {
    bytes: Vec<u8>,
    outcome: CreationOutcome,
    _memory: Arc<MemoryClaim>,
}
impl LaunchResult {
    /// Send these bounded bytes only through the original private engine channel.
    pub fn frame(&self) -> &[u8] {
        &self.bytes
    }
    /// Actual creation disposition; this is not application health or update acceptance.
    pub fn outcome(&self) -> CreationOutcome {
        self.outcome
    }
}

impl LaunchReceiver {
    /// Allocate inert custody bound to the original nonzero request identity before engine entry.
    /// # Errors
    /// Rejects an absent request identity; performs no native operation or execution admission.
    pub fn new(id: [u8; 32]) -> Result<Self, ClaimError> {
        if id == [0; 32] {
            return Err(ClaimError::invalid("launch_transfer_identity"));
        }
        Ok(Self(Arc::new(Mutex::new(Receiver {
            id,
            issued: false,
            handles: Vec::new(),
            memory: None,
            application: None,
            outcome: None,
            original: None,
        }))))
    }
    /// Duplicate source objects only from the exact retained authenticated engine process.
    /// Reserve `TRANSFER_RESERVATION_BYTES`. No paths are reopened and no process is started.
    /// Retain this owner and call `finish` even after failure or a cancelled worker result.
    /// # Errors
    /// Rejects replay, foreign/malformed frames, closed engine custody, non-disk inputs, native
    /// duplication failures, memory exhaustion and original worker cancellation/deadline expiry.
    pub fn capture(
        &self,
        engine: &ActivationLaunch,
        bytes: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        admit(context)?;
        engine.with_process(|process| self.capture_from(process, bytes, context))
    }
    /// Share the real duplication boundary with controlled native-handle tests.
    pub(in crate::application_launch) fn capture_from(
        &self,
        process: HANDLE,
        bytes: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        admit(context)?;
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued {
            return Err(ClaimError::invalid("launch_transfer_capture_closed"));
        }
        state.issued = true;
        state.original = Some(context.clone());
        let memory = Arc::new(
            context
                .memory()
                .claim(TRANSFER_RESERVATION_BYTES)
                .map_err(|cause| ClaimError::new("launch_transfer_memory", cause))?,
        );
        state.memory = Some(memory.clone());
        let request = wire::Request::decode(bytes, state.id)?;
        let entry = request.entry.handles();
        state
            .handles
            .try_reserve_exact(entry.len() + request.pins.len())
            .map_err(|cause| ClaimError::new("launch_transfer_native_outputs", cause))?;
        let budget = Budget::Protected(memory.clone());
        let mut inherited = [0; 2];
        for (index, raw) in entry.iter().enumerate() {
            admit(context)?;
            inherited[index] =
                native::duplicate(process, *raw, true, &mut state.handles, budget.clone())?;
        }
        for raw in &request.pins {
            admit(context)?;
            native::duplicate(process, *raw, false, &mut state.handles, budget.clone())?;
        }
        let command = request.command(&inherited[..entry.len()])?;
        admit(context)?;
        let application = ApplicationLaunch::new(command);
        crate::native_claims::lock(&application.0).custody =
            Some(LaunchCustody::Broker(Box::new(CapturedPins {
                handles: std::mem::take(&mut state.handles),
                inherited,
                count: entry.len(),
                _memory: memory,
            })));
        state.application = Some(application);
        Ok(())
    }
    /// Create exactly once through the ordinary independent-application boundary.
    /// Reserve `CREATION_RESERVATION_BYTES`; observe this job's terminal result before `finish`.
    /// # Errors
    /// Preserves absent capture, duplicate creation, native creation/close and admission failures.
    pub fn launch(&self, context: &JobContext) -> Result<(), ClaimError> {
        admit(context)?;
        let state = crate::native_claims::lock(&self.0);
        let original = state
            .original
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("launch_transfer_capture_absent"))?;
        admit(original)?;
        if original.owner_id() != context.owner_id() {
            return Err(ClaimError::invalid("launch_transfer_execution_owner"));
        }
        state
            .application
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("launch_transfer_application_absent"))?
            .launch(context)
    }
    /// Close admission and retire all native outputs before replying with actual creation status.
    /// Use an independent admitted cleanup job with `TRANSFER_RESERVATION_BYTES`; a failed close
    /// preserves its outputs and decision for retry under the original cleanup cutoff.
    /// # Errors
    /// Preserves original admission and checked native cleanup failures. Never fabricates a
    /// negative reply from cancellation, a missing engine or an uncertain native creation result.
    pub fn finish(&self, context: &JobContext) -> Result<LaunchResult, ClaimError> {
        admit(context)?;
        let mut state = crate::native_claims::lock(&self.0);
        if state.original.as_ref().is_some_and(|original| original.owner_id() != context.owner_id())
        {
            return Err(ClaimError::invalid("launch_transfer_execution_owner"));
        }
        state.issued = true;
        if let Some(application) = &state.application {
            state.outcome = Some(application.recover_and_retire(context)?);
            state.application = None;
        }
        files::close_all(&mut state.handles)?;
        let outcome = *state.outcome.get_or_insert(CreationOutcome::NotCreated);
        state.original = None;
        let bytes = wire::result(state.id, outcome)?;
        let memory = match state.memory.take() {
            Some(memory) => memory,
            None => Arc::new(
                context
                    .memory()
                    .claim(1024)
                    .map_err(|cause| ClaimError::new("launch_transfer_reply_memory", cause))?,
            ),
        };
        Ok(LaunchResult { bytes, outcome, _memory: memory })
    }
}

#[cfg(test)]
mod tests;
