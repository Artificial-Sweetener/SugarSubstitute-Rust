// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify one protected generation's retained manifest and pin every executable payload.
use super::bundle::VerifiedCopy;
use super::check;
use super::inventory;
use crate::durable_storage::{
    files::{self, Directory},
    protection,
};
use sha2::{Digest, Sha256};
use std::{fs::File, io};
use sugarsubstitute_contracts::{execution::JobContext, installation::Generation};
use sugarsubstitute_update::package::{MAX_MANIFEST_BYTES, Manifest};

/// Worker reservation for bounded inventory parsing, payload custody and native security queries.
pub const LAUNCH_RESERVATION_BYTES: u64 = 34 * 1024 * 1024;

/// Authenticated installed bytes without any application-run or trial authority.
pub(super) struct VerifiedApplication {
    pub(super) generation: Generation,
    pub(super) directory: Directory,
    pub(super) inventory: File,
    pub(super) payload: VerifiedCopy,
}
impl VerifiedApplication {
    /// Admit the inherited selection protocol only when the verified payload advertises it.
    pub(super) fn require_installed_launch(&self) -> io::Result<()> {
        self.payload.require_installed_launch()
    }
    /// Resolve only the recorded generation under its pinned installation, offline.
    /// Keep this owner alive while its executable is used; neither a path nor a hash pins bytes.
    pub(super) fn open(
        installation: &Directory,
        generation: Generation,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let _work = context.memory().claim(super::RESERVATION_BYTES).map_err(io::Error::other)?;
        let path = installation.path().join("releases").join(generation.id.as_str());
        let (directory, inventory_file, payload) = protection::with_domain(context, |protection| {
            protection.directory(
                &path,
                crate::durable_storage::protected::directories::Mode::Existing,
                context,
            )?;
            let directory = Directory::open(&path, false)?;
            let mut file = directory.read(inventory::INSTALLED_MANIFEST)?;
            protection.verify_file(&file)?;
            let bytes = files::read_bounded(&mut file, MAX_MANIFEST_BYTES)?;
            if format!("{:x}", Sha256::digest(&bytes)) != generation.package.as_str() {
                return Err(super::invalid(
                    "Generation inventory differs from protected selection",
                ));
            }
            let manifest = Manifest::decode(&bytes).map_err(io::Error::other)?;
            inventory::require_complete(&manifest)?;
            let payload = VerifiedCopy::open(&manifest, &path, context)?;
            Ok((directory, file, payload))
        })
        .map_err(protection::DomainFailure::into_io)?;
        check(context)?;
        Ok(Self { generation, directory, inventory: inventory_file, payload })
    }
}
