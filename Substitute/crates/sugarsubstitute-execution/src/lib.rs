// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded application scheduling, cancellation, result delivery and worker teardown.

mod admission;
mod admission_liveness;
mod admission_timing;
mod blocking;
#[cfg(any(test, feature = "boundary-fixtures"))]
#[doc(hidden)]
pub mod http_fixture;
mod owned_input;
/// Bounded transport for the owned upstream provisioning worker.
pub mod provisioning_transport;
pub use owned_input::{ChargedBlockingInput, OwnedInputError};
mod entry_preparation;
#[cfg(windows)]
mod windows_entry_environment;
#[cfg(any(windows, test))]
pub use entry_preparation::submission::EntryPreparationAdmissionRejection;
pub use entry_preparation::{
    CapturedEntryEnvironment, EntryEnvironmentFailure, EntryEnvironmentObservation,
    EntryPreparationCause, EntryPreparationContext, EntryPreparationControlCause,
    EntryPreparationError, EntryPreparationFailure, EntryPreparationInputError,
    EntryPreparationSuccess, EntryRequestAdoptionRejection, EntryRequestInput,
};
mod capture_bundle;
pub use capture_bundle::{CaptureBundleAdmission, CaptureBundleOrigin, CaptureBundleRetirement};
#[cfg(any(windows, test))]
mod capture_attach;
mod clock;
mod clock_retirement;
#[cfg(any(windows, test))]
mod counter_deadline;
mod cpu;
#[cfg(any(windows, test))]
mod desktop_capture;
mod fork_join;
#[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
mod preview_arming;
#[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
mod registered_desktop_fault;
#[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
pub use desktop_capture::desktop_capture_control_storage_bytes;
#[cfg(any(windows, test))]
pub use desktop_capture::{
    DESKTOP_CAPTURE_NATIVE_WORKSPACE_BYTES, DesktopCaptureAdmissionRejection,
    DesktopCaptureAttached, DesktopCaptureControl, DesktopCaptureSession,
    DesktopCaptureTemporaryState, InheritedCaptureAttachInput, NativeCaptureDriver,
    NativeCaptureInput, NativeCaptureProgress, NativeCaptureReply, NativeCaptureStep,
    ProcessLifetimeCapture,
};
#[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
pub use preview_arming::{PreviewArmingError, PreviewArmingSeal, PreviewSourceLoan};
#[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
pub use registered_desktop_fault::RegisteredDesktopFaultError;
#[cfg(any(windows, test))]
pub mod backend_custody;
#[cfg(any(windows, test))]
pub mod diagnostic_pipe;
#[cfg(any(windows, test))]
mod fixture_forwarding;
#[cfg(any(windows, test))]
mod fixture_session;
#[cfg(any(windows, test))]
mod lifecycle_pipe;
mod mailbox;
mod memory;
pub mod memory_budget;
mod native_resources;
#[cfg(any(windows, test))]
pub mod process_creation;
#[cfg(any(windows, test))]
pub mod recovery_supervisor;
mod symbol_provider;
pub use symbol_provider::{
    SymbolProviderAdmission, SymbolProviderOrigin, SymbolProviderRetirement,
};
mod symbol_bundle;
pub use symbol_bundle::{
    CommittedReaderAdmission, CommittedReaderOrigin, CommittedReaderRetirement,
    SymbolBundleAdmission, SymbolBundleOrigin, SymbolBundleRetirement,
};
mod report_export;
pub use report_export::{
    REPORT_EXPORT_BYTES, REPORT_EXPORT_CLEANUP_BYTES, REPORT_EXPORT_OUTPUTS, ReportExportAdmission,
    ReportExportCleanupBeginRejection, ReportExportCleanupDeadline, ReportExportCleanupPermit,
    ReportExportOrigin, ReportExportOutput, ReportExportPairJobs, ReportExportPairRequest,
    ReportExportPairSubmission, ReportExportRetirement, ReportExportSubmissionRejection,
};
mod symbol_admission;
mod symbol_cleanup;
pub use symbol_admission::{
    DiagnosticPipeResources, SymbolResourceRejection, SymbolSubmissionRejection,
    SymbolTransportReservation,
};
pub use symbol_cleanup::{CleanupDeadline, SymbolCleanupBeginRejection, SymbolCleanupPermit};
#[cfg(any(windows, test))]
mod native;
#[cfg(any(windows, test))]
mod native_deadlines;
#[cfg(any(windows, test))]
#[allow(unsafe_code)]
mod native_pipe;
#[cfg(any(windows, test))]
mod output;
mod runtime;
mod scope;
mod task;
mod worker_exit;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_fixture_forwarding;

/// Bounded, cancellable whole-file byte-range lock waits for blocking storage jobs.
#[cfg(windows)]
pub mod file_lock;
/// Bounded, cancellable waits for the first of several signalled kernel objects.
#[cfg(windows)]
pub mod handle_wait;
#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub mod windows_backend_custody;
#[cfg(windows)]
#[allow(unsafe_code)]
pub mod windows_capture_attach;
#[cfg(windows)]
pub mod windows_diagnostic_pipe;
#[cfg(windows)]
pub mod windows_fixture;
#[cfg(windows)]
pub mod windows_lifecycle_pipe;
#[cfg(windows)]
pub mod windows_output;
#[cfg(windows)]
pub mod windows_recovery_supervisor;

/// Deterministic external clock injection for tests using the real execution owner.
#[cfg(any(test, feature = "boundary-fixtures"))]
pub mod clock_fixture;

pub mod qualification_time;

pub use clock::MonotonicClock;
pub use runtime::{
    EntryEnvironmentCustody, EntryEnvironmentExtent, ExecutionClient, ExecutionLimits,
    ExecutionOwner, NativeOutputCustody, ProcessCaptureCustody, ShutdownProgress, UnresolvedJob,
    WorkerStatus,
};

#[cfg(test)]
mod runtime_test_support;

#[cfg(any(windows, test))]
mod coordinator_entry;
#[cfg(any(windows, test))]
pub use coordinator_entry::*;
#[cfg(windows)]
mod windows_coordinator_entry;

#[cfg(windows)]
mod windows_clock;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_counter;

#[cfg(test)]
mod symbol_test_support;
