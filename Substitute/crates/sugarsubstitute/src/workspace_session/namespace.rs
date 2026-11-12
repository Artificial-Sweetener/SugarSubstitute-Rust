// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep profile/workspace/backend correlation in host-owned bounded records.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use sugarsubstitute_contracts::{
    desktop_session::{BackendNamespaceId, InstallationId, ProfileId, WorkspaceId},
    lifecycle::BackendBindingDigest,
};
/// Maximum host-only index JSON bytes before decode.
const MAX_INDEX_BYTES: usize = 128 * 1024;
const MAX_BACKENDS: usize = 256;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    version: u32,
    installation: String,
    profile: String,
    workspace: String,
    backends: Vec<Binding>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Binding {
    digest: String,
    namespace: String,
}
/// Validated host-only mapping. A frontend checkpoint cannot create or alter this record.
pub struct NamespaceIndex {
    record: Record,
}
impl NamespaceIndex {
    /// Seed a new namespace only after selected installation/profile storage is settled.
    /// All identities are allocated by host owners; no browser UUID is accepted here.
    pub fn new(
        installation: &InstallationId,
        profile: &ProfileId,
        workspace: &WorkspaceId,
    ) -> Self {
        Self {
            record: Record {
                version: 1,
                installation: installation.as_str().into(),
                profile: profile.as_str().into(),
                workspace: workspace.as_str().into(),
                backends: vec![],
            },
        }
    }
    /// Decode bounded v1 bytes and compare their installation/profile with independent facts.
    /// Unsupported, foreign and corrupt originals must be preserved by the store owner.
    /// # Errors
    /// Refuses unknown/duplicate fields, versions, identities or repeated mappings.
    pub fn decode(
        bytes: &[u8],
        installation: &InstallationId,
        profile: &ProfileId,
    ) -> Result<Self, StoreError> {
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(StoreError::Protected);
        }
        let record: Record = serde_json::from_slice(bytes).map_err(|_| StoreError::Protected)?;
        if record.version != 1
            || record.installation != installation.as_str()
            || record.profile != profile.as_str()
            || record.backends.len() > MAX_BACKENDS
        {
            return Err(StoreError::Protected);
        }
        WorkspaceId::new(record.workspace.clone()).map_err(|_| StoreError::Protected)?;
        let mut digests = BTreeSet::new();
        let mut namespaces = BTreeSet::new();
        for binding in &record.backends {
            if binding.digest.len() != 64
                || !binding
                    .digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || !digests.insert(&binding.digest)
                || !namespaces.insert(&binding.namespace)
                || BackendNamespaceId::new(binding.namespace.clone()).is_err()
            {
                return Err(StoreError::Protected);
            }
        }
        Ok(Self { record })
    }
    /// Resolve an independently admitted target, preserving a known mapping exactly.
    /// A new target uses the host's fresh candidate namespace without replacing prior entries.
    /// # Errors
    /// Refuses exhausted mapping admission or reuse of a namespace for another backend.
    pub fn select(
        &mut self,
        digest: BackendBindingDigest,
        candidate: &BackendNamespaceId,
    ) -> Result<SessionScope, StoreError> {
        let digest = files::hex(digest.as_bytes());
        let namespace = match self.record.backends.iter().find(|binding| binding.digest == digest) {
            Some(binding) => binding.namespace.clone(),
            None => {
                if self.record.backends.len() >= MAX_BACKENDS
                    || self
                        .record
                        .backends
                        .iter()
                        .any(|binding| binding.namespace == candidate.as_str())
                {
                    return Err(StoreError::Protected);
                }
                let namespace = candidate.as_str().to_owned();
                self.record.backends.push(Binding { digest, namespace: namespace.clone() });
                namespace
            }
        };
        Ok(SessionScope {
            installation: self.record.installation.clone(),
            profile: self.record.profile.clone(),
            workspace: self.record.workspace.clone(),
            backend: namespace,
        })
    }
    /// Serialize the exact host record without changing its current authority or version.
    /// # Errors
    /// Preserves encoding or bounded-size admission failure.
    pub fn encode(&self) -> Result<Vec<u8>, StoreError> {
        let bytes = serde_json::to_vec(&self.record).map_err(io::Error::other)?;
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(StoreError::Protected);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
    fn id(number: u16) -> String {
        format!("00000000-0000-4000-8000-{number:012x}")
    }
    #[test]
    fn admitted_backend_reuses_namespace_after_restart_and_changed_identity_preserves_both()
    -> Result {
        let installation = InstallationId::new("1".repeat(32))?;
        let profile = ProfileId::new("real".into())?;
        let mut index = NamespaceIndex::new(&installation, &profile, &WorkspaceId::new(id(1))?);
        let original = index
            .select(BackendBindingDigest::from_bytes([1; 32]), &BackendNamespaceId::new(id(2))?)?;
        let mut index = NamespaceIndex::decode(&index.encode()?, &installation, &profile)?;
        let unchanged = index
            .select(BackendBindingDigest::from_bytes([1; 32]), &BackendNamespaceId::new(id(3))?)?;
        assert_eq!(unchanged, original);
        let changed = index
            .select(BackendBindingDigest::from_bytes([2; 32]), &BackendNamespaceId::new(id(3))?)?;
        assert_ne!(changed.backend, original.backend);
        assert_eq!(
            index.select(
                BackendBindingDigest::from_bytes([1; 32]),
                &BackendNamespaceId::new(id(4))?
            )?,
            original
        );
        Ok(())
    }
    #[test]
    fn foreign_scope_versions_duplicate_mapping_and_namespace_conflicts_are_protected() -> Result {
        let installation = InstallationId::new("1".repeat(32))?;
        let profile = ProfileId::new("real".into())?;
        let mut index = NamespaceIndex::new(&installation, &profile, &WorkspaceId::new(id(1))?);
        index
            .select(BackendBindingDigest::from_bytes([1; 32]), &BackendNamespaceId::new(id(2))?)?;
        assert!(
            index
                .select(BackendBindingDigest::from_bytes([2; 32]), &BackendNamespaceId::new(id(2))?)
                .is_err()
        );
        assert!(
            NamespaceIndex::decode(
                &index.encode()?,
                &installation,
                &ProfileId::new("Real".into())?
            )
            .is_err()
        );
        let mut bytes: serde_json::Value = serde_json::from_slice(&index.encode()?)?;
        bytes["version"] = 2.into();
        assert!(
            NamespaceIndex::decode(&serde_json::to_vec(&bytes)?, &installation, &profile).is_err()
        );
        bytes["version"] = 1.into();
        let duplicate = bytes["backends"][0].clone();
        bytes["backends"].as_array_mut().ok_or("Missing backend records")?.push(duplicate);
        assert!(
            NamespaceIndex::decode(&serde_json::to_vec(&bytes)?, &installation, &profile).is_err()
        );
        Ok(())
    }
}
