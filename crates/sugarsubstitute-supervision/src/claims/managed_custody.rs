// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project only the actual held generation into its original creation transaction.
use super::*;
use crate::{
    BackendCustodyParent,
    protected_storage::{exclusion::HeldFile, native_phase::NativePhase, paths, record_file},
};
use sugarsubstitute_contracts::execution::JobContext;

/// Strong source aliases retain their original claim and transport allocations.
#[derive(Clone)]
pub(crate) struct ManagedCustodySource {
    backend: Arc<Shared<Backend>>,
    parent: BackendCustodyParent,
    resource: ResourceKey,
}
impl ManagedCustodySource {
    /// Compose from actual private owners after canonical launch input validation.
    pub(super) fn new(entry: &BackendEntry, parent: &BackendCustodyParent) -> Self {
        Self { backend: entry.shared.clone(), parent: parent.clone(), resource: entry.resource }
    }
    /// Borrow the same original authenticated controller.
    pub(crate) const fn parent(&self) -> &BackendCustodyParent {
        &self.parent
    }
    /// Borrow immutable generation facts; these do not grant filesystem authority.
    pub(crate) const fn resource(&self) -> ResourceKey {
        self.resource
    }
    /// Read the exact held writer and record outside their source mutex.
    /// Returned bytes must match this actual generation and original custodian.
    pub(crate) fn preparation(
        &self,
        context: &JobContext,
    ) -> Result<CustodyPreparation, ClaimError> {
        self.parent.check().map_err(|cause| ClaimError::new("custody_creation_source", cause))?;
        let (file, expected, digest) = {
            let mut backend = lock(&self.backend.value);
            let expected = backend
                .record
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("custody_creation_record"))?
                .clone();
            if !backend.claimed
                || backend.released
                || backend.resource != self.resource
                || expected.version != 2
                || expected.custodian != self.parent.key().session().get()
                || expected.owner != self.parent.key().owner()
                || expected.process != 0
                || !matches!(
                    expected.state,
                    native_claims::RecordState::Held | native_claims::RecordState::LaunchPending
                )
            {
                return Err(ClaimError::invalid("custody_creation_generation"));
            }
            let file = backend
                .file
                .take()
                .ok_or_else(|| ClaimError::invalid("custody_creation_file_pending"))?;
            (file, expected, backend.proof.digest)
        };
        let loan = FileLoan { file: Some(file), source: self.clone() };
        let file =
            loan.file.as_ref().ok_or_else(|| ClaimError::invalid("custody_creation_file"))?;
        let observed = record_file::read_shape(file, context)?;
        if !matches!(observed, record_file::RecordRead::Present(bytes) if bytes == expected.encode())
        {
            return Err(ClaimError::invalid("custody_creation_changed_record"));
        }
        let budget = file.handle.budget();
        let identity = paths::file_identity(&file.handle, &mut NativePhase::ordinary(&budget)?)?;
        Ok(CustodyPreparation { record: expected.encode(), digest, writer: identity })
    }
}
/// Exact actual facts from the private held-generation source.
pub(crate) struct CustodyPreparation {
    pub(crate) record: [u8; 128],
    pub(crate) digest: [u8; 32],
    pub(crate) writer: [u8; 24],
}
/// Restore the same physical output on success, failure and unexpected unwind.
struct FileLoan {
    file: Option<HeldFile>,
    source: ManagedCustodySource,
}
impl Drop for FileLoan {
    fn drop(&mut self) {
        lock(&self.source.backend.value).file = self.file.take();
    }
}
