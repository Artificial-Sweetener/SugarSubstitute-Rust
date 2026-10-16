// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native implementation boundary; only finite execution jobs enter these methods.
use crate::{CaptureStartInput, CrashError};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    crash::CaptureHandlerIdentity,
    execution::{JobContext, OwnedJob},
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_execution::windows::{NativeObservationError, ProcessExit, ProcessWatchPermit};
use sugarsubstitute_execution::windows_capture_attach::{
    CaptureAttachExpectation, CaptureEndpointObservation,
};

mod bridge;
mod capture;
pub(crate) mod committed_parser;
pub(crate) mod desktop_driver;
pub(crate) mod finished_report;
pub(crate) mod incident_utc;
mod module_pin;
mod pins;
pub(crate) mod symbol_provider;

pub(crate) const CALLBACK_PIN_BYTES: usize = 64 * 1024;
pub(crate) type WatchJob = OwnedJob<ProcessExit, NativeObservationError>;
pub(crate) struct BindFailed {
    pub(crate) permit: ProcessWatchPermit,
    pub(crate) cause: CrashError,
}
/// Genuine process-local storage and process-PIN finished entry points move together.
pub(crate) struct RootStorageParts {
    pub(crate) storage: sugarsubstitute_supervision::ProtectedStorageRoot,
    api: finished_report::Api,
}
impl RootStorageParts {
    pub(crate) fn install(
        &self,
        report: &mut crate::incident::CapturedReport,
        context: &JobContext,
        execution: &sugarsubstitute_execution::ExecutionClient,
    ) -> Result<(), crate::incident::IncidentStoreError> {
        use crate::incident::IncidentStoreError;
        finished_report::Token::install(
            &mut report.token,
            self.api,
            &mut report.generation,
            &mut report.cpp,
            &mut report.rust,
            context,
        )
        .map_err(|_| IncidentStoreError::RecoveryConflict)?;
        report
            .token
            .as_mut()
            .ok_or(IncidentStoreError::RecoveryConflict)?
            .encode_deadline(execution)
            .map_err(|_| IncidentStoreError::RecoveryConflict)
    }
}
pub(crate) trait CaptureLease: Send {
    /// Move actual already-registered Root storage without native effects or a new claim.
    fn take_root_storage(&mut self) -> Result<RootStorageParts, CrashError>;
    fn prepare_finished(
        &mut self,
        report: &mut crate::incident::CapturedReport,
        storage: &mut sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorage,
        operation: &sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), crate::incident::IncidentStoreError>;
    fn start(
        &mut self,
        input: CaptureStartInput,
        owner: CoordinatorKey,
        context: &JobContext,
    ) -> Result<(), CrashError>;
    fn creator(&self) -> Option<crate::capture::CaptureCreator>;
    fn build_identity(&self) -> Option<&str>;
    fn handler(&self) -> Option<CaptureHandlerIdentity>;
    fn has_process(&self) -> bool;
    fn registered(&self) -> bool;
    fn endpoint(
        &self,
        expected: CaptureAttachExpectation,
    ) -> Result<CaptureEndpointObservation, CrashError>;
    fn callback_pin_bytes(&self) -> u64;
    fn callback_pin_reservation_bytes(&self) -> u64;
    fn bind_watch(&mut self, permit: ProcessWatchPermit) -> Result<WatchJob, BindFailed>;
    fn request_stop(&mut self) -> Result<(), CrashError>;
    fn retire(&mut self, context: &JobContext) -> Result<bool, CrashError>;
    fn take_error(&mut self) -> Option<CrashError>;
    fn take_omitted_errors(&mut self) -> u64;
}
pub(crate) trait NativePort: Send + Sync {
    fn create(
        &self,
        admission: sugarsubstitute_execution::CaptureBundleAdmission,
        execution: sugarsubstitute_execution::ExecutionClient,
    ) -> Result<Box<dyn CaptureLease>, CrashError>;
}
struct WindowsPort;
impl NativePort for WindowsPort {
    fn create(
        &self,
        admission: sugarsubstitute_execution::CaptureBundleAdmission,
        execution: sugarsubstitute_execution::ExecutionClient,
    ) -> Result<Box<dyn CaptureLease>, CrashError> {
        capture::Capture::new(admission, execution).map(|value| value as Box<dyn CaptureLease>)
    }
}
pub(crate) fn port() -> Arc<dyn NativePort> {
    Arc::new(WindowsPort)
}
pub(crate) fn check_deadline(context: &JobContext) -> Result<(), CrashError> {
    pins::deadline(context)
}

/// Identify only the current native clock observer for deterministic callbacks.
/// No thread handle, scheduler, creation, waiting or authority is exposed.
#[cfg(test)]
pub(crate) fn current_clock_observer() -> u32 {
    // SAFETY: This metadata-only Win32 call has no arguments or retained outputs.
    unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }
}

pub(crate) mod report_export;
