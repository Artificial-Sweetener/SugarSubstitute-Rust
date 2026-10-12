// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual storage owner against one bounded fake external filesystem.
//!
//! Available only to tests or the explicit, default-disabled `boundary-fixtures`
//! feature. The fixture supplies external observations; real storage methods mint
//! every operation, retained source and durable disposition authority.
pub(in crate::diagnostic_storage) mod boundary;
use super::*;
use boundary::Filesystem;

pub(in crate::diagnostic_storage) trait BoundaryClock:
    Send + Sync
{
    fn observe(&self, value: u64);
}

/// Choose a failure at an external primitive without modifying storage policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticFixtureFailure {
    /// Adopt a newly created output before returning failure.
    PartialCreate,
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
        let memory = Arc::new(
            context
                .memory()
                .claim(DIAGNOSTIC_STORAGE_BYTES)
                .map_err(|cause| failure(ClaimError::new("diagnostic_fixture_claim", cause)))?,
        );
        self.filesystem.retain_claim(&memory).map_err(failure)?;
        DiagnosticStorage::from_port(memory, Box::new(self.filesystem.clone()))
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
