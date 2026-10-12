// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain partial outputs before status and close only after actual kernel facts.
use super::{BindFailed, CaptureLease, WatchJob, bridge, pins};
use crate::{CaptureStartInput, CrashError, input::STORAGE_BYTES};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use sugarsubstitute_contracts::{
    crash::{CaptureHandlerIdentity, CaptureOperation},
    execution::{JobContext, MemoryClaim, MemoryLease},
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_execution::windows::ProcessWatchPermit;
use windows::Win32::{
    Foundation::{DuplicateHandle, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        Threading::*,
    },
};

struct Fields {
    pins: pins::Pins,
    storage: sugarsubstitute_supervision::ProtectedStorageRoot,
    process: Option<usize>,
    process_exited: bool,
    api: Option<bridge::Api>,
    pin_claim: Option<MemoryClaim>,
    _storage: MemoryClaim,
    handler: Option<CaptureHandlerIdentity>,
    registered: bool,
    stop_requested: bool,
    io_retired: bool,
    errors: [Option<CrashError>; 9],
    omitted: u64,
}
pub(super) struct Capture {
    fields: Option<Box<Fields>>,
}
impl Capture {
    pub(super) fn new(memory: &MemoryLease) -> Result<Box<Self>, CrashError> {
        // Both actual claims precede every native effect/allocation owner.
        let storage = memory
            .claim((STORAGE_BYTES - super::CALLBACK_PIN_BYTES - 1024 * 1024) as u64)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let pin_claim = memory
            .claim(super::CALLBACK_PIN_BYTES as u64)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let storage_claim = memory
            .claim(1024 * 1024)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let protected = sugarsubstitute_supervision::ProtectedStorageRoot::new(storage_claim)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        Ok(Box::new(Self {
            fields: Some(Box::new(Fields {
                pins: pins::Pins::new()?,
                storage: protected,
                process: None,
                process_exited: false,
                api: None,
                pin_claim: Some(pin_claim),
                _storage: storage,
                handler: None,
                registered: false,
                stop_requested: false,
                io_retired: false,
                errors: std::array::from_fn(|_| None),
                omitted: 0,
            })),
        }))
    }
    fn fields(&mut self) -> Result<&mut Fields, CrashError> {
        self.fields
            .as_mut()
            .map(Box::as_mut)
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Retirement))
    }
}
impl Fields {
    fn retain(&mut self, cause: CrashError) {
        if let Some(slot) = self.errors.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    fn close_report(&mut self, report: pins::CloseReport) -> Option<CrashError> {
        let first = report.first;
        if let Some(cause) = &first {
            self.retain(cause.clone());
        }
        for cause in report.additional.into_iter().flatten() {
            self.retain(cause);
        }
        self.omitted = self.omitted.saturating_add(report.omitted);
        first
    }
    fn cleanup_codes(&mut self, output: &bridge::ResultValue) -> Result<(), CrashError> {
        if output.cleanup_count > 8 {
            return Err(CrashError::invalid(CaptureOperation::Retirement));
        }
        for code in &output.cleanup_codes[..output.cleanup_count as usize] {
            self.retain(native_code(*code, CaptureOperation::Retirement));
        }
        self.omitted = self.omitted.saturating_add(output.omitted_cleanup);
        Ok(())
    }
}
fn native_code(code: u32, operation: CaptureOperation) -> CrashError {
    CrashError::new(operation, std::io::Error::from_raw_os_error(code as i32))
}
fn identity(handle: HANDLE) -> Result<CaptureHandlerIdentity, CrashError> {
    // SAFETY: The primary actual CreateProcess output remains uniquely retained.
    let process_id = unsafe { GetProcessId(handle) };
    if process_id == 0 {
        return Err(CrashError::new(
            CaptureOperation::Observation,
            windows::core::Error::from_thread(),
        ));
    }
    let mut birth = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: Actual retained process and four initialized FILETIME outputs.
    unsafe { GetProcessTimes(handle, &mut birth, &mut exit, &mut kernel, &mut user) }
        .map_err(|cause| CrashError::new(CaptureOperation::Observation, cause))?;
    let creation_time_100ns =
        (u64::from(birth.dwHighDateTime) << 32) | u64::from(birth.dwLowDateTime);
    if creation_time_100ns == 0 {
        return Err(CrashError::invalid(CaptureOperation::Observation));
    }
    Ok(CaptureHandlerIdentity { process_id, creation_time_100ns })
}
fn exited(fields: &Fields) -> Result<bool, CrashError> {
    if fields.process_exited {
        return Ok(true);
    }
    let Some(index) = fields.process else {
        return Ok(true);
    };
    // SAFETY: One zero-time query of this actual retained kernel process; no
    // scheduler/wait registration or worker occupation is created here.
    let observed = unsafe { WaitForSingleObject(fields.pins.handle(index)?, 0) };
    if observed == WAIT_OBJECT_0 {
        Ok(true)
    } else if observed == WAIT_TIMEOUT {
        Ok(false)
    } else {
        Err(CrashError::new(CaptureOperation::Observation, windows::core::Error::from_thread()))
    }
}
fn retire_io(fields: &mut Fields) -> Result<bool, CrashError> {
    let Some(api) = fields.api else {
        fields.io_retired = true;
        return Ok(true);
    };
    let slots = fields.pins.reserve_outputs()?;
    let output = api.retire();
    // Retire never manufactures new processes, but adopt any unexpected partial
    // ABI output before validating the response or interpreting its status.
    let adoption = fields.pins.retain_outputs(slots, [output.process, output.thread]);
    fields.pins.release_unused_outputs();
    adoption?;
    if output.version != 1 || output.retired > 1 || output.pending_io > 1 {
        return Err(CrashError::invalid(CaptureOperation::Retirement));
    }
    fields.io_retired = output.retired == 1;
    fields.cleanup_codes(&output)?;
    Ok(fields.io_retired)
}
impl CaptureLease for Capture {
    fn start(
        &mut self,
        input: &CaptureStartInput,
        owner: CoordinatorKey,
        context: &JobContext,
    ) -> Result<(), CrashError> {
        let fields = self.fields()?;
        let (client, client_index) = fields.pins.resolve(&input.client, false, context)?;
        fields.pins.verify(client_index, input.client_hash, context)?;
        let (handler, handler_index) = fields.pins.resolve(&input.handler, false, context)?;
        fields.pins.verify(handler_index, input.handler_hash, context)?;
        fields
            .storage
            .resolve(&input.capture_root, context)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        fields
            .storage
            .create_capture_generation(owner, context)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let generation = fields
            .storage
            .generation()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
        let database = pins::wide(generation.database_path())?;
        let metrics = pins::wide(generation.metrics_path())?;
        pins::deadline(context)?;
        let client = pins::wide(&client)?;
        let claim = fields
            .pin_claim
            .take()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))?;
        let api = match bridge::load(&client, claim) {
            Ok(api) => api,
            Err((cause, claim)) => {
                fields.pin_claim = claim;
                return Err(cause);
            }
        };
        fields.api = Some(api);
        let handler = pins::wide(&handler)?;
        let mut coordinator = [0u8; 96];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (index, byte) in owner.namespace.bytes().iter().chain(owner.epoch.bytes()).enumerate() {
            coordinator[index * 2] = HEX[(byte >> 4) as usize];
            coordinator[index * 2 + 1] = HEX[(byte & 15) as usize];
        }
        let mut frequency = 0i64;
        // SAFETY: Initialized integer metadata outputs have the exact QPF ABI.
        unsafe { QueryPerformanceFrequency(&mut frequency) }
            .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?;
        if frequency <= 0 || frequency > i64::MAX / 1000 {
            return Err(CrashError::invalid(CaptureOperation::Registration));
        }
        let before = context.now();
        let mut counter = 0i64;
        // SAFETY: Initialized integer metadata output has the exact QPC ABI.
        unsafe { QueryPerformanceCounter(&mut counter) }
            .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?;
        let after = context.now();
        if after < before || counter < 0 {
            return Err(CrashError::invalid(CaptureOperation::Registration));
        }
        let remaining = context
            .deadline()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))?
            .checked_duration_since(after)
            .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?;
        let ticks = i64::try_from(remaining.as_nanos() * frequency as u128 / 1_000_000_000)
            .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))?;
        let deadline_counter = counter
            .checked_add(ticks)
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))?;
        pins::deadline(context)?;
        let slots = fields.pins.reserve_outputs()?;
        let output = api.start(&bridge::Input {
            handler: handler.as_ptr(),
            handler_length: handler.len() - 1,
            database: database.as_ptr(),
            database_length: database.len() - 1,
            metrics: metrics.as_ptr(),
            metrics_length: metrics.len() - 1,
            build: input.build.as_ptr(),
            build_length: input.build.len(),
            coordinator: coordinator.as_ptr(),
            coordinator_length: coordinator.len(),
            deadline_counter,
            counter_frequency: frequency,
        });
        // This precedes every ABI status/version/error check, including failure.
        let adoption = fields.pins.retain_outputs(slots, [output.process, output.thread]);
        if fields.pins.values[slots[0]].is_some() {
            fields.process = Some(slots[0]);
        }
        fields.pins.release_unused_outputs();
        adoption?;
        let identity_error = if let Some(index) = fields.process {
            match identity(fields.pins.handle(index)?) {
                Ok(value) => {
                    fields.handler = Some(value);
                    None
                }
                Err(cause) => Some(cause),
            }
        } else {
            None
        };
        if let Some(cause) = &identity_error {
            fields.retain(cause.clone());
        }
        fields.cleanup_codes(&output)?;
        if output.version != 1 || output.registered > 1 || output.pending_io > 1 {
            return Err(CrashError::invalid(CaptureOperation::Registration));
        }
        fields.registered = output.registered == 1;
        if output.error != 0 {
            return Err(native_code(output.error, CaptureOperation::Registration));
        }
        if let Some(cause) = identity_error {
            return Err(cause);
        }
        if output.cleanup_error != 0 {
            return Err(native_code(output.cleanup_error, CaptureOperation::Retirement));
        }
        pins::deadline(context)?;
        if !fields.registered || fields.handler.is_none() {
            return Err(CrashError::invalid(CaptureOperation::Registration));
        }
        Ok(())
    }
    fn handler(&self) -> Option<CaptureHandlerIdentity> {
        self.fields.as_ref().and_then(|fields| fields.handler)
    }
    fn has_process(&self) -> bool {
        self.fields.as_ref().is_some_and(|fields| fields.process.is_some())
    }
    fn registered(&self) -> bool {
        self.fields.as_ref().is_some_and(|fields| fields.registered)
    }
    fn callback_pin_bytes(&self) -> u64 {
        bridge::pinned_bytes()
    }
    fn callback_pin_reservation_bytes(&self) -> u64 {
        bridge::pinned_reservation_bytes()
    }
    fn bind_watch(&mut self, permit: ProcessWatchPermit) -> Result<WatchJob, BindFailed> {
        let duplicate = (|| {
            let fields = self.fields()?;
            let index =
                fields.process.ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?;
            let mut duplicate = HANDLE::default();
            let slots = fields.pins.reserve_outputs()?;
            // SAFETY: Process pseudo-handles remain valid; the retained primary
            // is duplicated non-inheritably with query/synchronize rights only.
            let status = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    fields.pins.handle(index)?,
                    GetCurrentProcess(),
                    &mut duplicate,
                    PROCESS_QUERY_LIMITED_INFORMATION.0 | PROCESS_SYNCHRONIZE.0,
                    false,
                    windows::Win32::Foundation::DUPLICATE_HANDLE_OPTIONS(0),
                )
            };
            let adoption = fields.pins.retain_outputs(slots, [duplicate.0 as usize, 0]);
            let adopted = fields.pins.values[slots[0]].is_some().then_some(slots[0]);
            fields.pins.release_unused_outputs();
            adoption?;
            status.map_err(|cause| CrashError::new(CaptureOperation::Observation, cause))?;
            let index =
                adopted.ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?;
            let raw = fields.pins.values[index]
                .take()
                .ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?;
            // SAFETY: This unique owned duplicate transfers exactly once to the
            // execution owner, which observes callback quiescence before closure.
            Ok(unsafe { OwnedHandle::from_raw_handle(raw as *mut _) })
        })();
        match duplicate {
            Ok(handle) => Ok(permit.bind(handle)),
            Err(cause) => Err(BindFailed { permit, cause }),
        }
    }
    fn request_stop(&mut self) -> Result<(), CrashError> {
        let fields = self.fields()?;
        if fields.stop_requested || exited(fields)? {
            return Ok(());
        }
        let index = fields.process.ok_or_else(|| CrashError::invalid(CaptureOperation::Stop))?;
        // SAFETY: Only this actual retained owned handler is terminated, after
        // the caller's explicit capture-window policy authorizes logical close.
        unsafe { TerminateProcess(fields.pins.handle(index)?, 1) }
            .map_err(|cause| CrashError::new(CaptureOperation::Stop, cause))?;
        fields.stop_requested = true;
        Ok(())
    }
    fn retire(&mut self, context: &JobContext) -> Result<bool, CrashError> {
        let fields = self.fields()?;
        let io = retire_io(fields)?;
        if !io || !exited(fields)? {
            return Ok(false);
        }
        fields.process_exited = true;
        let report = fields.pins.close();
        if let Some(cause) = fields.close_report(report) {
            return Err(cause);
        }
        fields
            .storage
            .retire(context)
            .map_err(|cause| CrashError::new(CaptureOperation::Retirement, cause))?;
        // Preserve bounded errors until the owner drains them; closed handles and
        // scratch remain charged for this final delivery, independently of the PIN.
        fields.io_retired = true;
        Ok(true)
    }
    fn take_error(&mut self) -> Option<CrashError> {
        self.fields.as_mut().and_then(|fields| fields.errors.iter_mut().find_map(Option::take))
    }
    fn take_omitted_errors(&mut self) -> u64 {
        self.fields.as_mut().map_or(0, |fields| std::mem::take(&mut fields.omitted))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(fields) = self.fields.take() {
            // Only explicit admitted jobs perform bridge retirement/checked closure.
            // The existing durable state is retained without allocating or issuing FFI.
            if !fields.pins.is_empty()
                || (fields.api.is_some() && !fields.io_retired)
                || !fields.storage.is_retired()
            {
                // Only a specific unresolved native operation/failed close retains
                // these actual owners. It does not claim logical/native settlement.
                retain_before_diagnostics(fields, |retained| {
                    for cause in retained.errors.iter().flatten() {
                        tracing::error!(operation = "capture_owner_loss_retirement", cause = %cause, "Retained native cleanup cause");
                    }
                    tracing::error!(
                        operation = "capture_owner_loss_retirement",
                        "Unresolved native ownership retains bounded real storage and charge"
                    );
                });
            }
        }
    }
}

fn retain_before_diagnostics<T>(owned: Box<T>, diagnostics: impl FnOnce(&T)) {
    // Irrevocable retention precedes any subscriber or Display implementation.
    // Box::leak consumes the existing allocation and performs no FFI/allocation.
    diagnostics(Box::leak(owned));
}

#[cfg(test)]
mod tests;
