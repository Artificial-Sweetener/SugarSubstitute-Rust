// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read existing committed records through genuine namespace and paired execution owners.
pub(crate) mod authority;
mod owner;
mod prepare;
mod read;
mod request;
mod retire;
mod types;
use crate::{
    ClaimError,
    durable_storage::protected::{
        exclusion::HeldFile,
        native_file::{NativeCauseDestinations, NativeFile},
        native_name::{NativeName, NativeSid},
    },
    installed_release::{
        AllocationOrigin, CaptureCapacityFailure, CaptureOutputQuota, CaptureScratch,
        SharedCaptureLoan,
    },
    native_claims::files::Handle,
};
pub use owner::CommittedDiagnosticReader;
pub use request::ProtectedCommittedReadRequest;
use std::cell::RefCell;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_execution::CommittedReaderOrigin;
pub use types::{
    CommittedReadObject, CommittedReadSlot, CommittedReaderConstructionFailure,
    CommittedReaderRetirementFailure,
};
fn missing() -> ClaimError {
    ClaimError::capture_allocation(CaptureCapacityFailure::missing())
}
fn phase<'a>(
    original: &'a AllocationOrigin,
    scratch: &'a mut CaptureScratch,
    causes: &'a mut NativeCauseDestinations,
) -> crate::durable_storage::protected::native_phase::NativePhase<'a> {
    crate::durable_storage::protected::native_phase::NativePhase::Selected {
        original,
        scratch,
        causes: RefCell::new(causes),
    }
}
struct PreparedFile {
    output: crate::installed_release::CaptureOutputGuard,
    causes: NativeCauseDestinations,
}
/// Four permanent chain pins plus one still-owned canonical verification output.
const DIRECTORY_PINS: usize = 4 + 1;
struct Fields {
    request: ProtectedCommittedReadRequest,
    namespace: crate::coordinator_entry::read_namespace::NamespaceReadLoan,
    root: Option<NativeName>,
    directory: Option<NativeName>,
    sid: Option<NativeSid>,
    directories: Vec<Handle>,
    directory_causes: [NativeCauseDestinations; DIRECTORY_PINS],
    token: Handle,
    token_causes: NativeCauseDestinations,
    writer: Option<HeldFile<1>>,
    writer_output: Option<PreparedFile>,
    files: [Option<NativeFile>; 3],
    prepared: [Option<PreparedFile>; 3],
    objects: [Option<types::CommittedReadObject>; 3],
    quota: CaptureOutputQuota,
    scratch: CaptureScratch,
    causes: NativeCauseDestinations,
    attempted: bool,
    ready: bool,
    failure: Option<ClaimError>,
}
impl Fields {
    fn context(&mut self, original: &AllocationOrigin) -> Result<JobContext, ClaimError> {
        original
            .original()
            .original_context()
            .cloned()
            .map_err(|cause| self.causes.selected_control(cause, false))
    }
    fn has_output(&self) -> bool {
        self.token.is_armed()
            || self.directories.iter().any(Handle::is_armed)
            || self.writer.as_ref().is_some_and(|writer| writer.handle.is_armed())
            || self.files.iter().flatten().any(NativeFile::has_output)
    }
}

#[cfg(test)]
mod tests;
