// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind complete import receipts to their original logical and native installation identities.
use super::ImportAssociation;
use crate::durable_storage::{files, invalid};
use serde::{Deserialize, Serialize};
use std::{io, path::Path};
use sugarsubstitute_contracts::{configuration::ProfileId, installation::InstallationId};

pub(super) const MAX_ENTRIES: u64 = 8192;
pub(super) const MAX_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub(super) const MAX_RECORD_BYTES: usize = 256 * 1024;

/// Persist distinct installed and source-host semantics without promoting correlation to ownership.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Association {
    Installed { installation_id: String },
    SourceHost { correlation: String, profile: String, namespace: [u8; 32] },
}
impl Association {
    fn new(value: ImportAssociation<'_>) -> Self {
        match value {
            ImportAssociation::Installed(identity) => {
                Self::Installed { installation_id: identity.as_str().into() }
            }
            ImportAssociation::SourceHost { correlation, profile, namespace } => Self::SourceHost {
                correlation: correlation.as_str().into(),
                profile: profile.as_str().into(),
                namespace: *namespace.bytes(),
            },
        }
    }
    fn valid(&self) -> bool {
        match self {
            Self::Installed { installation_id } => {
                InstallationId::new(installation_id.clone()).is_ok()
            }
            Self::SourceHost { correlation, profile, namespace } => {
                InstallationId::new(correlation.clone()).is_ok()
                    && ProfileId::validate(profile).is_ok()
                    && *namespace != [0; 32]
            }
        }
    }
}

/// The feature supplies verified existing correlation; paths create no identity or authority.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
    schema_version: u32,
    association: Association,
    pub(super) installation: (u32, u32, u32),
    pub(super) source: (u32, u32, u32),
    parent: (u32, u32, u32),
    pub(super) destination: String,
}
impl Journal {
    /// Completed data remains usable independently of later edits or removal of its old source.
    pub(super) fn belongs_to(
        &self,
        association: ImportAssociation<'_>,
        installation: (u32, u32, u32),
        parent: (u32, u32, u32),
        destination: &Path,
    ) -> bool {
        self.association == Association::new(association)
            && self.installation == installation
            && self.parent == parent
            && destination.to_str() == Some(self.destination.as_str())
    }

    /// Capture the actual root/source/parent association before creating any stage.
    pub(super) fn new(
        association: ImportAssociation<'_>,
        installation: (u32, u32, u32),
        source: (u32, u32, u32),
        parent: (u32, u32, u32),
        destination: &Path,
    ) -> io::Result<Self> {
        let association = Association::new(association);
        if !association.valid() {
            return Err(invalid("Import correlation is not a complete typed association"));
        }
        Ok(Self {
            schema_version: 1,
            association,
            installation,
            source,
            parent,
            destination: destination
                .to_str()
                .ok_or_else(|| invalid("Import path is not Unicode"))?
                .to_owned(),
        })
    }
}

/// Stable content and native-source digests; hierarchy boundaries are part of both streams.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tree {
    pub(super) entries: u64,
    pub(super) bytes: u64,
    pub(super) sha256: [u8; 32],
    pub(super) identities: [u8; 32],
}
impl Tree {
    /// Destination objects have new native identities but must preserve every admitted byte.
    pub(super) fn same_contents(&self, other: &Self) -> bool {
        (self.entries, self.bytes, self.sha256) == (other.entries, other.bytes, other.sha256)
    }
}

/// Publish this complete record before the matching directory can become active.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    pub(super) journal: Journal,
    pub(super) tree: Tree,
    pub(super) stage: String,
    pub(super) stage_identity: (u32, u32, u32),
}
impl Receipt {
    /// Reject unsupported or ambiguous records without changing their original bytes.
    pub(super) fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid("Import receipt exceeds admission"));
        }
        let value: Self = serde_json::from_slice(bytes).map_err(io::Error::other)?;
        if value.journal.schema_version != 1
            || !value.journal.association.valid()
            || value.journal.destination.len() > 128 * 1024
            || files::lexical(Path::new(&value.journal.destination)).is_err()
            || value.tree.entries > MAX_ENTRIES
            || value.tree.bytes > MAX_BYTES
            || !value.stage.starts_with(".user-import-")
            || value.stage.len() > 128
            || files::flat_leaf(&value.stage).is_err()
        {
            return Err(invalid("Unsupported historical import receipt"));
        }
        Ok(value)
    }
}
