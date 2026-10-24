// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain accepted updater execution inputs independently of diagnostic symbol availability.
use super::{VerifiedRuntime, manifest::AuthenticatedManifest};
use crate::setup_storage::{check, files::Directory, inventory, protection::Protection};
use std::{fs::File, io, path::Path};
use sugarsubstitute_contracts::{execution::JobContext, update::Component};

/// Authenticated runtime files have launch authority but cannot supply complete package copies.
pub(in crate::setup_storage) struct RuntimePackage {
    directory: Directory,
    manifest: File,
    pub(in crate::setup_storage) runtime: VerifiedRuntime,
}

impl RuntimePackage {
    /// Reopen an already accepted updater using its protected package identity.
    /// One storage reservation funds inventory parsing and retained runtime pins.
    /// Missing diagnostic files are permitted; unknown files, unsafe paths and runtime damage fail.
    pub(in crate::setup_storage) fn open(
        root: &Path,
        expected: &str,
        context: &JobContext,
    ) -> io::Result<Self> {
        let memory =
            context.memory().claim(super::super::RESERVATION_BYTES).map_err(io::Error::other)?;
        let manifest = AuthenticatedManifest::open(root, expected, Component::Updater, context)?;
        let runtime = VerifiedRuntime::open_admitted(
            &manifest.inventory,
            &root.join("app"),
            context,
            memory,
        )?;
        runtime.compatibility.require_updater()?;
        inventory::verify_runtime(root, &manifest.inventory, || check(context))?;
        Ok(Self { directory: manifest.directory, manifest: manifest.file, runtime })
    }

    /// The helper's working directory stays pinned for its complete process-family lifetime.
    pub(in crate::setup_storage) fn directory(&self) -> &Path {
        &self.directory.path
    }

    /// Require installed account protection on the inventory and each execution input.
    pub(in crate::setup_storage) fn require_protected(
        &self,
        protection: &Protection,
    ) -> io::Result<()> {
        protection.verify_file(&self.manifest)?;
        self.runtime.require_protected(protection)
    }
}
