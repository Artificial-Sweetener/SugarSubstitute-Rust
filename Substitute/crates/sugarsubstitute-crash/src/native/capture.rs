// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain partial outputs before status and close only after actual kernel facts.
use super::{BindFailed, CaptureLease, WatchJob, bridge, pins};
use crate::{CaptureStartInput, CrashError, input::NATIVE_STORAGE_BYTES};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use sugarsubstitute_contracts::{
    crash::{CaptureHandlerIdentity, CaptureOperation},
    execution::{JobContext, MemoryClaim},
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_execution::windows::ProcessWatchPermit;
use windows::Win32::{
    Foundation::{DuplicateHandle, FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::*,
};

mod finished;
mod registration;
mod retention;
use retention::native_code;

struct Fields {
    pins: pins::Pins,
    execution: sugarsubstitute_execution::ExecutionClient,
    admission: Option<sugarsubstitute_execution::CaptureBundleAdmission>,
    original: Option<sugarsubstitute_execution::CaptureBundleOrigin>,
    selected: Option<sugarsubstitute_supervision::installed_release::CaptureBundleOwner>,
    lease: Option<sugarsubstitute_supervision::installed_release::CaptureBundleLease>,
    creator: Option<crate::capture::CaptureCreator>,
    parts: Option<Box<sugarsubstitute_supervision::installed_release::CaptureBundleParts>>,
    input: Option<CaptureStartInput>,
    build: [u8; 64],
    build_length: usize,
    storage: Option<sugarsubstitute_supervision::ProtectedStorageRoot>,
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
    pub(super) fn new(
        admission: sugarsubstitute_execution::CaptureBundleAdmission,
        execution: sugarsubstitute_execution::ExecutionClient,
    ) -> Result<Box<Self>, CrashError> {
        let context = admission
            .original_context()
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?
            .clone();
        let memory = context.memory();
        // The source owner already retains its separate 16KiB claim. These three
        // claims cover actual native owners before any allocation or effect.
        let storage = memory
            .claim(NATIVE_STORAGE_BYTES as u64)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let pin_claim = memory
            .claim(super::CALLBACK_PIN_BYTES as u64)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let storage_claim = memory
            .claim(1024 * 1024)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let protected = sugarsubstitute_supervision::ProtectedStorageRoot::new(storage_claim)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        const _: () = assert!(
            size_of::<Fields>()
                + size_of::<Capture>()
                + size_of::<crate::capture::Shared>()
                + size_of::<crate::capture::root_storage::Owner>()
                + 4 * size_of::<usize>()
                + 64
                < 16 * 1024
        );
        Ok(Box::new(Self {
            fields: Some(Box::new(Fields {
                pins: pins::Pins::new()?,
                execution,
                admission: Some(admission),
                original: None,
                selected: None,
                lease: None,
                creator: None,
                parts: None,
                input: None,
                build: [0; 64],
                build_length: 0,
                storage: Some(protected),
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
    fn prepare_finished(
        &mut self,
        report: &mut crate::incident::CapturedReport,
        storage: &mut sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorage,
        operation: &sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), crate::incident::IncidentStoreError> {
        self.prepare_finished_native(report, storage, operation, context)
    }
    fn start(
        &mut self,
        input: CaptureStartInput,
        owner: CoordinatorKey,
        context: &JobContext,
    ) -> Result<(), CrashError> {
        registration::register(self.fields()?, input, owner, context)
    }
    fn take_root_storage(&mut self) -> Result<crate::native::RootStorageParts, CrashError> {
        let fields = self.fields()?;
        if !fields.registered
            || fields.stop_requested
            || fields.io_retired
            || fields.input.as_ref().is_none_or(|input| input.recovery.is_some())
        {
            return Err(CrashError::invalid(CaptureOperation::Observation));
        }
        let api = fields
            .api
            .as_ref()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?
            .finished;
        let storage = fields
            .storage
            .take()
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?;
        Ok(crate::native::RootStorageParts { storage, api })
    }
    fn creator(&self) -> Option<crate::capture::CaptureCreator> {
        self.fields.as_ref().and_then(|fields| fields.creator)
    }
    fn build_identity(&self) -> Option<&str> {
        self.fields
            .as_ref()
            .and_then(|fields| std::str::from_utf8(&fields.build[..fields.build_length]).ok())
            .filter(|value| !value.is_empty())
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
    fn endpoint(
        &self,
        expected: sugarsubstitute_execution::windows_capture_attach::CaptureAttachExpectation,
    ) -> Result<
        sugarsubstitute_execution::windows_capture_attach::CaptureEndpointObservation,
        CrashError,
    > {
        let fields = self
            .fields
            .as_ref()
            .filter(|fields| fields.registered && !fields.stop_requested)
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?;
        fields
            .api
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?
            .endpoint(expected)
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
        if let Some(storage) = &mut fields.storage {
            storage
                .retire(context)
                .map_err(|cause| CrashError::new(CaptureOperation::Retirement, cause))?;
        }
        if let Some(original) = fields.original.as_ref()
            && (fields.selected.is_some() || fields.lease.is_some() || fields.parts.is_some())
        {
            let proof = fields
                .execution
                .capture_bundle_retirement(original, context)
                .map_err(|cause| CrashError::new(CaptureOperation::Retirement, cause))?;
            if let Some(owner) = fields.selected.take()
                && let Err(failure) = owner.retire(&proof)
            {
                fields.selected = Some(failure.owner);
                return Err(CrashError::new(CaptureOperation::Retirement, failure.cause));
            }
            if let Some(parts) = fields.parts.take() {
                let sugarsubstitute_supervision::installed_release::CaptureBundleParts {
                    retention,
                    loaded,
                    pinned,
                } = *parts;
                match retention.retire(&proof) {
                    Ok(()) => {
                        drop(loaded);
                        drop(pinned);
                    }
                    Err(failure) => {
                        fields.parts = Some(Box::new(
                            sugarsubstitute_supervision::installed_release::CaptureBundleParts {
                                retention: failure.retention,
                                loaded,
                                pinned,
                            },
                        ));
                        return Err(CrashError::new(CaptureOperation::Retirement, failure.cause));
                    }
                }
            }
        }
        if fields.lease.is_some() {
            // A rejected consuming lease remains genuine held physical custody.
            return Ok(false);
        }
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
            if fields.selected.is_some()
                || fields.lease.is_some()
                || fields.parts.is_some()
                || !fields.pins.is_empty()
                || (fields.api.is_some() && !fields.io_retired)
                || fields.storage.as_ref().is_some_and(|storage| !storage.is_retired())
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
