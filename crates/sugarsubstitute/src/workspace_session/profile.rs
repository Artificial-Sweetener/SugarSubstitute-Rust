// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize a selected profile's namespace index and authoritative asset reservations.
use super::*;
use std::io::Read;
use sugarsubstitute_contracts::{
    desktop_session::{
        BackendNamespaceId, InstallationId, ProfileId, WorkspaceId, wire::SessionAsset,
    },
    execution::{JobContext, MemoryClaim},
    lifecycle::BackendBindingDigest,
};
use sugarsubstitute_supervision::{
    ArtifactError, ArtifactPublication, with_artifact_input, write_artifact,
};
const INDEX: &str = "namespace-index.json";
const INVENTORY: &str = "asset-inventory.json";
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

/// One selected installation/profile. The existing native profile owner excludes other writers.
/// Call this owner only after install migration and selected profile storage have settled.
#[derive(Clone)]
pub struct ProfileStore {
    directory: PathBuf,
    installation: InstallationId,
    profile: ProfileId,
}
/// A host-selected durable namespace, with its profile-wide reservation owner retained.
pub struct OpenedProfile {
    /// Complete trusted scope, passed to the current frontend bootstrap after native admission.
    pub scope: SessionScope,
    /// Store for this exact independently admitted backend namespace.
    pub workspace: WorkspaceStore,
    profile: ProfileStore,
    inventory: AssetInventory,
    inventory_bytes: Vec<u8>,
    usable: bool,
    _memory: MemoryClaim,
}
impl ProfileStore {
    /// Compose only `user/sessions/<validated profile>` from the installation layout owner's
    /// absolute user directory. A page or checkpoint cannot select this root.
    /// # Errors
    /// Refuses relative or ambiguous ancestry before any I/O.
    pub fn new(
        user: &Path,
        installation: InstallationId,
        profile: ProfileId,
    ) -> Result<Self, StoreError> {
        if !user.is_absolute()
            || user.components().any(|part| {
                matches!(part, std::path::Component::ParentDir | std::path::Component::CurDir)
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Profile session root must be absolute",
            )
            .into());
        }
        Ok(Self { directory: user.join("sessions").join(profile.as_str()), installation, profile })
    }
    /// Borrow the host-selected profile directory; this carries no native handles.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// Working memory for bounded host records and native storage scratch.
    pub const fn reservation_bytes() -> u64 {
        128 * 1024 * 1024
    }
    /// Select/reuse a backend namespace through one finite execution job. Random UUIDs come
    /// from the native storage owner; they are never inferred from frontend presentation.
    /// # Errors
    /// Preserves execution admission rejection before any I/O or UUID allocation.
    #[cfg(windows)]
    pub fn submit_open<P: ExecutionPort>(
        &self,
        execution: &P,
        scope: &Scope,
        spec: JobSpec,
        binding: BackendBindingDigest,
    ) -> Result<OwnedJob<OpenedProfile, StoreError>, AdmissionError> {
        let store = self.clone();
        execution.submit_blocking(scope, spec, move |context| store.open_worker(binding, &context))
    }
    pub(crate) fn open_worker(
        &self,
        binding: BackendBindingDigest,
        context: &JobContext,
    ) -> Result<OpenedProfile, StoreError> {
        let workspace =
            WorkspaceId::new(sugarsubstitute_supervision::new_artifact_identity(context)?)?;
        let backend =
            BackendNamespaceId::new(sugarsubstitute_supervision::new_artifact_identity(context)?)?;
        self.open(binding, &workspace, &backend, context)
    }
    fn open(
        &self,
        binding: BackendBindingDigest,
        candidate_workspace: &WorkspaceId,
        candidate_backend: &BackendNamespaceId,
        context: &JobContext,
    ) -> Result<OpenedProfile, StoreError> {
        let memory = context.memory().claim(64 * 1024 * 1024).map_err(io::Error::other)?;
        let (mut index, original, mut protected) =
            match read_validated(&self.directory, INDEX, context, |bytes| {
                NamespaceIndex::decode(bytes, &self.installation, &self.profile)
            })? {
                Some((index, bytes, protected)) => (index, Some(bytes), protected),
                None => (
                    NamespaceIndex::new(&self.installation, &self.profile, candidate_workspace),
                    None,
                    false,
                ),
            };
        let scope = index.select(binding, candidate_backend)?;
        let bytes = index.encode()?;
        if original.as_deref() != Some(bytes.as_slice()) {
            if protected {
                return Err(StoreError::Protected);
            }
            publish_record(&self.directory, INDEX, original.as_deref(), &bytes, context)?;
        }
        let (inventory, inventory_bytes) =
            match read_validated(&self.directory, INVENTORY, context, |bytes| {
                AssetInventory::decode(bytes, &self.installation, &self.profile)
            })? {
                Some((inventory, original, invalid)) => {
                    protected |= invalid;
                    (inventory, original)
                }
                None => {
                    let inventory = AssetInventory::new(&self.installation, &self.profile);
                    let bytes = inventory.encode()?;
                    if !protected {
                        publish_record(&self.directory, INVENTORY, None, &bytes, context)?;
                    }
                    (inventory, bytes)
                }
            };
        let directory = self
            .directory
            .join("workspaces")
            .join(&scope.workspace)
            .join("backends")
            .join(&scope.backend);
        let workspace =
            WorkspaceStore::new(directory, self.directory.join("assets"), scope.clone())?;
        Ok(OpenedProfile {
            scope,
            workspace,
            profile: self.clone(),
            inventory,
            inventory_bytes,
            usable: !protected,
            _memory: memory,
        })
    }
}
impl OpenedProfile {
    /// Whether preserved invalid/future host records require read-only recovery. The host
    /// propagates this to checkpoint publication even when a verified previous mapping loads.
    pub fn protected(&self) -> bool {
        !self.usable
    }
    /// Persist a quota reservation before publishing an exact verified source. Runs only on
    /// the profile's serialized ordinary storage worker. Pending reservations survive failure.
    /// # Errors
    /// Preserves quota, source identity, storage and native retirement failures.
    pub fn retain_asset(
        &mut self,
        asset: &SessionAsset,
        source: &mut dyn Read,
        context: &JobContext,
    ) -> Result<(), StoreError> {
        if !self.usable {
            return Err(StoreError::Protected);
        }
        let original = &self.inventory_bytes;
        self.inventory.reserve(asset)?;
        let reserved = self.inventory.encode()?;
        if *original != reserved {
            self.usable = false;
            publish_record(&self.profile.directory, INVENTORY, Some(original), &reserved, context)?;
            self.usable = true;
        }
        self.inventory_bytes = reserved.clone();
        assets::retain(&self.workspace.assets, asset, source, context)?;
        self.inventory.verified(asset)?;
        let verified = self.inventory.encode()?;
        if reserved != verified {
            self.usable = false;
            publish_record(
                &self.profile.directory,
                INVENTORY,
                Some(&reserved),
                &verified,
                context,
            )?;
            self.usable = true;
        }
        self.inventory_bytes = verified;
        Ok(())
    }
}
fn read_validated<T>(
    directory: &Path,
    name: &str,
    context: &JobContext,
    decode: impl Fn(&[u8]) -> Result<T, StoreError>,
) -> Result<Option<(T, Vec<u8>, bool)>, StoreError> {
    let mut protected = false;
    for leaf in [name.to_owned(), previous(name)] {
        let bytes = match with_artifact_input(
            &directory.join(leaf),
            MAX_RECORD_BYTES as u64,
            context,
            |input| {
                let _memory = context.memory().claim(input.len()).map_err(io::Error::other)?;
                let mut bytes = Vec::new();
                input.read_to_end(&mut bytes)?;
                Ok::<_, io::Error>(bytes)
            },
        ) {
            Ok(bytes) => bytes,
            Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {
                continue;
            }
            Err(cause) => return Err(files::artifact(cause)),
        };
        match decode(&bytes) {
            Ok(record) => return Ok(Some((record, bytes, protected))),
            Err(StoreError::Protected) => protected = true,
            Err(cause) => return Err(cause),
        }
    }
    if protected { Err(StoreError::Protected) } else { Ok(None) }
}
/// Load primary or, if absent, previous. Invalid/future records remain protected rather than
/// deriving new UUIDs. Candidate validators run before any possible record publication.
fn read_record(
    directory: &Path,
    name: &str,
    context: &JobContext,
) -> Result<Option<Vec<u8>>, StoreError> {
    for leaf in [name.to_owned(), previous(name)] {
        let result =
            with_artifact_input(&directory.join(leaf), MAX_RECORD_BYTES as u64, context, |input| {
                let _memory = context.memory().claim(input.len()).map_err(io::Error::other)?;
                let mut bytes = Vec::new();
                bytes.try_reserve_exact(input.len() as usize).map_err(io::Error::other)?;
                input.read_to_end(&mut bytes)?;
                Ok::<_, io::Error>(bytes)
            });
        match result {
            Ok(bytes) => return Ok(Some(bytes)),
            Err(ArtifactError::Storage(cause)) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(files::artifact(cause)),
        }
    }
    Ok(None)
}
fn previous(name: &str) -> String {
    format!("{}.previous.json", name.trim_end_matches(".json"))
}
/// Publish a validated host record. Original expected bytes are re-read before preserving
/// them; caller validation, profile exclusion and monotonic execution serialize all writers.
fn publish_record(
    directory: &Path,
    name: &str,
    expected: Option<&[u8]>,
    bytes: &[u8],
    context: &JobContext,
) -> Result<(), StoreError> {
    let actual = read_record(directory, name, context)?;
    if actual.as_deref() != expected {
        return Err(StoreError::Changed);
    }
    if let Some(original) = expected {
        write_artifact(
            &directory.join(previous(name)),
            true,
            ArtifactPublication::ReplaceSelected,
            original.len().max(1),
            context,
            |output| output.write_all(original),
        )
        .map_err(files::artifact)?;
    }
    write_artifact(
        &directory.join(name),
        true,
        ArtifactPublication::ReplaceSelected,
        bytes.len().max(1),
        context,
        |output| output.write_all(bytes),
    )
    .map_err(files::artifact)
}
