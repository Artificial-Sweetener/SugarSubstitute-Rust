// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded launch inputs and own contained Windows process families.
//!
//! Prepared input buffers establish lexical shape and argument fidelity. Native
//! callers separately establish canonical trust and authenticated launch authority.
//! The Windows registry retains native families through cancellation and observed cleanup.

#[cfg(windows)]
pub mod backend_listener;
#[cfg(windows)]
pub mod installed_frontend;
#[cfg(windows)]
pub mod installed_release;
#[cfg(windows)]
pub mod session_identity;
#[cfg(windows)]
pub mod setup_storage;
#[cfg(windows)]
pub use setup_storage::cache;
#[cfg(windows)]
pub mod update_http;
#[cfg(windows)]
pub mod update_preparation;
#[cfg(windows)]
pub mod update_repository;

mod arguments;
mod command;
mod errors;
mod release_selection;
pub use release_selection::{
    CaptureSelectionRequest, InstalledReleaseOwner, SymbolSelectionRequest,
};

#[cfg(windows)]
mod claim_error;
#[cfg(windows)]
mod claim_types;
#[cfg(windows)]
mod claims;
#[cfg(all(windows, feature = "boundary-fixtures"))]
#[doc(hidden)]
pub use claims::boundary_fixture as claim_boundary_fixture;
#[cfg(windows)]
#[allow(unsafe_code)]
pub mod application_launch;
#[cfg(windows)]
pub mod committed_reader;
#[cfg(windows)]
pub mod coordinator_entry;
#[cfg(windows)]
#[allow(unsafe_code)]
mod creation_error_mode;
#[cfg(windows)]
pub mod diagnostic_storage;
#[cfg(any(windows, unix))]
mod durable_storage;
#[cfg(any(windows, unix))]
pub use durable_storage::artifacts::{
    ArtifactBytes, ArtifactError, ArtifactInput, ArtifactPublication, read_artifact,
    with_artifact_input, write_artifact,
};
#[cfg(windows)]
pub use durable_storage::artifacts::{
    ArtifactReadPermit, RetainedArtifactReader, new_artifact_identity,
};
#[cfg(windows)]
mod environment;
#[cfg(windows)]
pub mod fixture_descendant;
#[cfg(windows)]
pub mod lifecycle_protocol;
#[cfg(windows)]
mod managed_output;
#[cfg(windows)]
#[allow(unsafe_code)]
mod native_claims;
#[cfg(windows)]
#[allow(unsafe_code)]
pub mod profile_conflict;
#[cfg(windows)]
pub mod profiles;
#[cfg(windows)]
pub use durable_storage::input::{PreparationFileError, read_preparation_file};
#[cfg(windows)]
pub use durable_storage::owned_retirement::{
    OwnedRoot, RETIREMENT_RESERVATION_BYTES, Retired, RetirementError, RetirementOutcome,
    RetirementReport, RetirementUnit, Tombstone, TreeInventory,
};
#[cfg(windows)]
pub use durable_storage::protected::{ProtectedStorageGeneration, ProtectedStorageRoot};
#[cfg(windows)]
pub use durable_storage::receiving::{
    ReportFile, ReportFileCode, ReportFileCustody, ReportFileError, ReportFileSelection,
};
#[cfg(windows)]
pub use durable_storage::records::{
    RECORD_PAGE_ENTRIES, RecordBytes, RecordDirectory, RecordEntry, WORKER_RECORD_BYTES,
};
#[cfg(windows)]
#[allow(unsafe_code)]
mod native_process;
#[cfg(all(windows, feature = "k1-native-qualification"))]
#[allow(unsafe_code)]
pub mod qualification_artifact;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod registry_error;
#[cfg(windows)]
mod registry_input;
#[cfg(windows)]
mod registry_types;

pub use command::{MAX_WINDOWS_ARGUMENTS, WindowsLaunchBuffers, WindowsLaunchCommand};
pub use errors::LaunchCommandError;

#[cfg(windows)]
pub use claim_error::{ClaimAdmissionError, ClaimError};
#[cfg(windows)]
mod backend_custody_parent;
#[cfg(windows)]
pub use backend_custody_parent::{
    BackendCustodyAcknowledgement, BackendCustodyFinishStatus, BackendCustodyParent,
    BackendCustodyParentFailure, BackendCustodyParentInputError,
};
#[cfg(windows)]
pub use claim_types::{
    CLAIM_POOL_BYTES, ClaimLaunchJobs, ClaimObservation, ClaimTarget, ManagedLaunchInput,
    ResolutionTicket,
};
#[cfg(windows)]
pub use claims::{CLAIM_OPERATION_BYTES, WindowsResourceClaims};
#[cfg(windows)]
pub use environment::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};
#[cfg(windows)]
pub use managed_output::RejectedManagedOutputLaunch;
#[cfg(windows)]
pub use native_claims::backend_custody::{
    BackendCustodyDriverInputError, BackendCustodyExitDiagnostic, BackendCustodyFailure,
    BackendCustodyInstallation, BackendCustodySource,
};
#[cfg(windows)]
pub use registry::{SettledRegistryFailure, SettledUpdaterProbe, WindowsProcessRegistry};
#[cfg(windows)]
pub use registry_error::{RegistryAdmissionError, RegistryError, RegistryInputError};
#[cfg(windows)]
pub use registry_input::{RejectedOutputLaunch, WindowsLaunchInput};
#[cfg(windows)]
pub use registry_types::{
    FamilyBinding, LaunchTicket, RegistryObservation, RegistrySnapshot, RootObservationOutcome,
};

#[cfg(windows)]
pub use registry::{
    RecoveryDesktopWatch, RecoveryEnrolledDesktop, RecoveryHandlerWatch,
    RecoveryHandlerWatchRejection,
};

#[cfg(windows)]
pub use registry::{
    InheritedRecoveryJobInput, RecoveryChildCreation, RecoveryChildCreationRejection,
    RecoveryChildInput, RecoveryChildInputRejection, RecoveryChildStatus,
};

#[cfg(windows)]
pub use registry_input::ProvisioningEndpoint;

#[cfg(windows)]
pub use registry::SettledProvisioning;
