// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist authoritative desktop checkpoints below an installation-owned profile namespace.
//!
//! Storage and digest work runs only on the supplied execution owner's ordinary workers.
//! Namespace correlation is not filesystem or backend authority. The production composition
//! supplies installation/profile ownership and a fresh independently admitted backend binding.
mod assets;
mod baseline;
#[cfg(windows)]
pub mod close_record;
mod driver;
mod files;
mod identity;
mod namespace;
mod outbound;
#[cfg(windows)]
mod profile;
mod transfer;
pub use assets::AssetInventory;
pub use baseline::CheckpointBaseline;
#[cfg(all(windows, any(feature = "native-composition", test)))]
pub(crate) use driver::NativeOriginalResolution;
pub use driver::{DriverEvent, RestoreFailure, WorkspaceDriver};
pub use namespace::NamespaceIndex;
pub use outbound::EncodedMaterialization;
#[cfg(windows)]
pub use profile::{OpenedProfile, ProfileStore};
/// Correlate a canonical admitted external attachment without claiming its process or files.
/// The existing origin owner provides host spelling/port normalization; no path rules are added.
#[cfg(all(windows, feature = "windows-native-host"))]
pub(crate) fn attached_binding(
    origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
) -> sugarsubstitute_contracts::lifecycle::BackendBindingDigest {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    let tag = b"SugarSubstitute.attached-session.v1";
    hash.update((tag.len() as u64).to_le_bytes());
    hash.update(tag);
    hash.update([match origin.host() {
        sugarsubstitute_contracts::configuration::LoopbackHost::Ipv4 => 1,
        sugarsubstitute_contracts::configuration::LoopbackHost::Ipv6 => 2,
        sugarsubstitute_contracts::configuration::LoopbackHost::Localhost => 3,
    }]);
    hash.update(origin.port().get().to_le_bytes());
    sugarsubstitute_contracts::lifecycle::BackendBindingDigest::from_bytes(hash.finalize().into())
}
pub use transfer::{CaptureTransfer, CapturedWorkspace, TransferError};
#[cfg(all(test, windows))]
mod tests;
use std::{
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{
    desktop_session::{
        MAX_CHECKPOINT_BYTES, SessionError,
        wire::{SessionCheckpoint, SessionScope},
    },
    execution::{AdmissionError, ExecutionPort, JobSpec, OwnedJob, Scope},
};

/// Current authoritative checkpoint leaf.
pub const CURRENT_CHECKPOINT: &str = "workspace.json";
/// Last verified replaced checkpoint retained for recovery.
pub const PREVIOUS_CHECKPOINT: &str = "workspace.previous.json";

/// Storage refusal retaining the original cause and all previously published state.
#[derive(Debug)]
pub enum StoreError {
    /// Portable checkpoint validation failed.
    Contract(SessionError),
    /// Future, foreign, oversized or unassociated originals inhibit publication.
    Protected,
    /// An immutable object no longer matches its recorded content identity.
    Changed,
    /// Actual I/O, admission-control or checked native retirement failed.
    Storage(io::Error),
}
impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(cause) => write!(formatter, "Workspace checkpoint rejected: {cause}"),
            Self::Protected => {
                formatter.write_str("Workspace originals require compatible recovery before saving")
            }
            Self::Changed => {
                formatter.write_str("Workspace artifact changed from its recorded identity")
            }
            Self::Storage(_) => {
                formatter.write_str("Workspace storage failed; original cause retained")
            }
        }
    }
}
impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(cause) => Some(cause),
            Self::Storage(cause) => Some(cause),
            Self::Protected | Self::Changed => None,
        }
    }
}
impl From<io::Error> for StoreError {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}
impl From<SessionError> for StoreError {
    fn from(cause: SessionError) -> Self {
        Self::Contract(cause)
    }
}

/// Which owned checkpoint supplied restored data or rejection evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointFile {
    /// Current published checkpoint.
    Current,
    /// Previous verified publication.
    Previous,
}
/// A candidate retained byte-for-byte because its data could not be admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedCheckpoint {
    /// Owned leaf that was rejected.
    pub file: CheckpointFile,
    /// Original bytes' SHA256, suitable for recovery correlation, never path authority.
    pub digest: String,
    /// Whether this candidate prohibits automatic replacement.
    pub protected: bool,
}
/// Startup load result; missing media is resolved separately by the output owner.
#[derive(Debug)]
pub struct RestoredWorkspace {
    /// Decoded current or previous checkpoint.
    pub checkpoint: Option<(CheckpointFile, SessionCheckpoint)>,
    /// Rejected candidates, still untouched on disk.
    pub rejected: Vec<RejectedCheckpoint>,
    /// Automatic publication is prohibited even if recovery data was usable.
    pub protected: bool,
    _memory: Option<sugarsubstitute_contracts::execution::MemoryClaim>,
}
/// Actual successful checked atomic publication, after validated referenced assets exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveReceipt {
    /// Published immutable checkpoint identity.
    pub checkpoint_id: String,
    /// Published host-assigned durable revision.
    pub revision: u64,
    /// Published JSON byte length.
    pub bytes: u64,
    /// Whether a valid current publication became the previous checkpoint.
    pub kept_previous: bool,
    /// Digest-named original-byte evidence created before replacing rejected data.
    pub evidence: Vec<PathBuf>,
}
/// Inert store for one host-selected profile/workspace/backend directory.
#[derive(Clone, Debug)]
pub struct WorkspaceStore {
    directory: PathBuf,
    assets: PathBuf,
    scope: SessionScope,
}
impl WorkspaceStore {
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn load_worker(
        &self,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<RestoredWorkspace, StoreError> {
        files::load(self, context)
    }
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn save_worker(
        &self,
        checkpoint: &SessionCheckpoint,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<SaveReceipt, StoreError> {
        files::save(self, checkpoint, context)
    }
    /// Select an absolute installation-owned directory and independently trusted scope.
    /// The composition, not checkpoint/page data, selects this directory.
    /// # Errors
    /// Refuses relative or ambiguous ancestry and malformed namespace identities.
    pub fn new(
        directory: PathBuf,
        assets: PathBuf,
        scope: SessionScope,
    ) -> Result<Self, StoreError> {
        use sugarsubstitute_contracts::desktop_session::{
            BackendNamespaceId, InstallationId, ProfileId, WorkspaceId,
        };
        InstallationId::new(scope.installation.clone()).map_err(io::Error::other)?;
        ProfileId::new(scope.profile.clone()).map_err(io::Error::other)?;
        WorkspaceId::new(scope.workspace.clone())?;
        BackendNamespaceId::new(scope.backend.clone())?;
        if [&directory, &assets].iter().any(|path| {
            !path.is_absolute()
                || path.components().any(|part| {
                    matches!(part, std::path::Component::ParentDir | std::path::Component::CurDir)
                })
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Workspace directory must be absolute and unambiguous",
            )
            .into());
        }
        Ok(Self { directory, assets, scope })
    }
    #[cfg(all(windows, feature = "native-composition"))]
    pub(crate) fn verify_asset(
        &self,
        asset: &sugarsubstitute_contracts::desktop_session::wire::SessionAsset,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<bool, StoreError> {
        match assets::verify(&self.assets, asset, context) {
            Ok(()) => Ok(true),
            Err(StoreError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(cause) => Err(cause),
        }
    }
    #[cfg(all(windows, any(feature = "native-composition", test)))]
    pub(crate) fn asset_path(
        &self,
        asset: &sugarsubstitute_contracts::desktop_session::wire::SessionAsset,
    ) -> Result<PathBuf, StoreError> {
        if !asset.valid()
            || asset.digest.len() != 64
            || !asset.digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(StoreError::Protected);
        }
        Ok(self.assets.join(format!("{}.{}", asset.digest, asset.extension)))
    }
    /// Borrow the host-selected storage directory, without granting native handles.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Bounded working admission for JSON bytes, decoded owned strings and encoding validation.
    pub const fn reservation_bytes() -> u64 {
        (MAX_CHECKPOINT_BYTES as u64) * 12 + 4 * 1024 * 1024
    }
    /// Read current, then previous if missing/rejected, on an execution-owned finite job.
    /// # Errors
    /// Preserves queue/memory/scope admission rejection before touching storage.
    pub fn submit_load<P: ExecutionPort>(
        &self,
        execution: &P,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<OwnedJob<RestoredWorkspace, StoreError>, AdmissionError> {
        let store = self.clone();
        execution.submit_blocking(scope, spec, move |context| files::load(&store, &context))
    }
    /// Publish an immutable validated checkpoint after its referenced asset/input receipts.
    /// A single profile publisher must serialize calls; this owner refuses non-increasing disk revisions.
    /// # Errors
    /// Preserves execution admission rejection before touching storage.
    pub fn submit_save<P: ExecutionPort>(
        &self,
        execution: &P,
        scope: &Scope,
        spec: JobSpec,
        checkpoint: SessionCheckpoint,
    ) -> Result<OwnedJob<SaveReceipt, StoreError>, AdmissionError> {
        let store = self.clone();
        execution
            .submit_blocking(scope, spec, move |context| files::save(&store, &checkpoint, &context))
    }
}
