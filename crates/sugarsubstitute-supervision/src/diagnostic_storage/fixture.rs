// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual storage owner against one bounded fake external filesystem.
//!
//! Available only to tests or the explicit, default-disabled `boundary-fixtures`
//! feature. The fixture supplies external observations; real storage methods mint
//! every operation, retained source and durable disposition authority.
pub(in crate::diagnostic_storage) mod boundary;
use super::*;
pub use crate::durable_storage::protected::capture_read::fixture::ProtectedCaptureSourceFixture;
use boundary::Filesystem;

pub(in crate::diagnostic_storage) trait BoundaryClock:
    Send + Sync
{
    fn observe(&self, value: u64);
}

#[cfg(test)]
pub(in crate::diagnostic_storage) mod native_probe;
/// Choose a failure at an external primitive without modifying storage policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticFixtureFailure {
    /// Adopt a newly created output before returning failure.
    PartialCreate,
    /// Write actual bytes before returning an external failure.
    PartialWrite,
    /// Fail checked actual flush without publication proof.
    Flush,
    /// Adopt an opened output before returning failure.
    PartialOpen,
    /// Return bytes differing from the retained file digest.
    ReadCorruption,
    /// Keep the actual output live when checked close fails.
    Close,
    /// Rename the stage before returning an activation error.
    PostActivation,
    /// Fail the actual protected absence observation.
    AbsenceObservation,
    /// Return an incomplete physical scan.
    IncompleteScan,
    /// Open a retained inventory output before reporting changed identity.
    ChangedInventory,
}

/// Retain one bounded external disk shared by actual storage owner instances.
pub struct DiagnosticStorageFixture {
    filesystem: Filesystem,
}
impl DiagnosticStorageFixture {
    /// Allocate bounded fake external state without effects or execution authority.
    /// # Errors
    /// Returns capacity failure if the initial observation allocation fails.
    pub fn new() -> Result<Self, DiagnosticStorageError> {
        let filesystem = Filesystem::new();
        filesystem.reserve().map_err(failure)?;
        Ok(Self { filesystem })
    }
    /// Claim the real 8MiB budget and install the actual storage algorithms.
    /// The caller retains this owner before beginning or resolving an operation.
    /// # Errors
    /// Rejects insufficient admitted memory or another live owner of this disk.
    pub fn storage(
        &self,
        context: &JobContext,
    ) -> Result<DiagnosticStorage, DiagnosticStorageError> {
        let memory = context
            .memory()
            .claim(DIAGNOSTIC_STORAGE_BYTES)
            .map_err(|cause| failure(ClaimError::new("diagnostic_fixture_claim", cause)))?;
        let allocation = allocation::owner(memory)?;
        let filesystem = self.filesystem.retain_claim(&allocation).map_err(failure)?;
        DiagnosticStorage::from_port(allocation, OwnedPort::new(Box::new(filesystem), None))
    }
    /// Resolve the same retained external filesystem and return actual opening disposition.
    /// This boundary-fixture method runs the real storage owner against its admitted
    /// disk; it cannot mint proof for another fixture or a production native port.
    /// # Errors
    /// Rejects foreign fixture/storage/original operation and original control failure.
    pub fn resolve_opening(
        &self,
        storage: &mut DiagnosticStorage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticStoreOpening, DiagnosticStorageError> {
        let mut state = storage.check(operation, context)?;
        let marker = self.filesystem.marker();
        if !state.data_mut()?.allocation.matches_fixture(&marker)
            || state.data_mut()?.namespace.is_some()
        {
            return Err(error(operation.key(), Code::InvalidIdentity));
        }
        let opening = state
            .call_native(|port| {
                port.resolve(
                    Path::new("external-fixture-only"),
                    operation.key().owner().namespace,
                    context,
                )
            })?
            .map_err(|cause| native(operation.key(), Code::ForeignProtection, cause))?;
        storage.adopt_opening(state, opening, operation, context)
    }
    /// Observe a complete externally supplied file under its typed canonical name.
    /// Existing content at that name is replaced as an external observation.
    /// # Errors
    /// Rejects object/physical byte limits or bounded observation capacity.
    pub fn put(
        &self,
        object: DiagnosticObject,
        bytes: &[u8],
    ) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
        self.filesystem.put_checked(object, None, bytes).map_err(failure)
    }
    /// Observe the exact historical stage inode without creating storage authority.
    /// # Errors
    /// Rejects foreign historical namespace, byte limits or observation capacity.
    pub fn put_stage(
        &self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        bytes: &[u8],
    ) -> Result<DiagnosticReadFacts, DiagnosticStorageError> {
        self.filesystem.put_checked(object, Some(historical), bytes).map_err(failure)
    }
    /// Observe an external prior deletion; this never grants a deletion permit.
    /// # Errors
    /// Rejects mutation of an inode still retained by an actual fake output.
    pub fn erase_existing(&self, object: DiagnosticObject) -> Result<(), DiagnosticStorageError> {
        self.filesystem.erase_checked(object).map_err(failure)
    }
    /// Set only an external primitive failure; owner state remains inaccessible.
    pub fn set_failure(&self, failure: DiagnosticFixtureFailure, enabled: bool) {
        self.filesystem.set_failure(failure, enabled);
    }
    /// Observe live external outputs and effects without minting native proofs.
    pub fn snapshot(&self) -> DiagnosticFixtureSnapshot {
        self.filesystem.snapshot()
    }
    #[cfg(test)]
    pub(in crate::diagnostic_storage) fn file_observations(&self) -> usize {
        self.filesystem.file_observations()
    }
}

/// Immutable external observations with no operation or native authority.
pub struct DiagnosticFixtureSnapshot {
    live_outputs: usize,
    mutation_count: usize,
    active: Vec<(DiagnosticObject, DiagnosticReadFacts)>,
}
impl DiagnosticFixtureSnapshot {
    /// Observe actual output slots still awaiting checked close.
    pub const fn live_outputs(&self) -> usize {
        self.live_outputs
    }
    /// Observe actual create, write, rename and delete attempts that took effect.
    pub const fn mutation_count(&self) -> usize {
        self.mutation_count
    }
    /// Observe one active canonical file's actual identity, digest and byte count.
    pub fn active_identity(&self, object: DiagnosticObject) -> Option<DiagnosticReadFacts> {
        self.active.iter().find(|(target, _)| *target == object).map(|(_, facts)| *facts)
    }
}
fn failure(cause: ClaimError) -> DiagnosticStorageError {
    DiagnosticStorageError::native(None, Code::Admission, cause)
}
