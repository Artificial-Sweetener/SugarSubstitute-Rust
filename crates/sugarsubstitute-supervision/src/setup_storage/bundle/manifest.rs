// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate a retained package inventory before either complete-copy or runtime admission.
use super::super::{check, invalid, inventory};
use crate::durable_storage::files::{self, Directory};
use sha2::{Digest, Sha256};
use std::{fs::File, io, path::Path};
use sugarsubstitute_contracts::{execution::JobContext, update::Component};
use sugarsubstitute_update::package::{MAX_MANIFEST_BYTES, Manifest};

/// Exact manifest and directory handles remain pinned while their decoded inventory is consumed.
/// The caller reserves storage capacity before reading or parsing any bytes.
pub(super) struct AuthenticatedManifest {
    pub(super) directory: Directory,
    pub(super) file: File,
    pub(super) bytes: Vec<u8>,
    pub(super) digest: String,
    pub(super) inventory: Manifest,
}

impl AuthenticatedManifest {
    /// Match the inventory to trusted installer or protected selection identity and component.
    pub(super) fn open(
        root: &Path,
        expected: &str,
        component: Component,
        context: &JobContext,
    ) -> io::Result<Self> {
        Self::open_directory(Directory::open(root, false)?, expected, component, context)
    }

    /// Authenticate the exact directory already checked against its cache lease or external source.
    pub(super) fn open_directory(
        directory: Directory,
        expected: &str,
        component: Component,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid("Package inventory authority is invalid"));
        }
        let mut file = directory.read("setup-manifest.json")?;
        let bytes = files::read_bounded(&mut file, MAX_MANIFEST_BYTES)?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        if digest != expected {
            return Err(invalid("Package inventory authentication failed"));
        }
        let inventory = Manifest::decode(&bytes).map_err(io::Error::other)?;
        inventory::require_component(&inventory, component)?;
        check(context)?;
        Ok(Self { directory, file, bytes, digest, inventory })
    }
}
