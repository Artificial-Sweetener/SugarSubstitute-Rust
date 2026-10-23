// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rehash and pin staged application payloads against their retained authenticated manifest.
use super::super::check;
use super::super::invalid;
use super::payload::{Input, hash};
use crate::durable_storage::files::Directory;
use std::{io, path::Path};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_update::package::Manifest;

/// Exact destination bytes and all resolved ancestors retained against write/replacement.
pub(in crate::setup_storage) struct VerifiedCopy {
    pub(in crate::setup_storage) compatibility: super::super::compatibility::Compatibility,
    _inputs: Vec<Input>,
    _memory: MemoryClaim,
}

impl VerifiedCopy {
    /// Verify the staged application rather than inferring completion from an earlier copy call.
    /// Existing package custody authenticates the manifest; these handles pin its destination.
    pub(in crate::setup_storage) fn open(
        inventory: &Manifest,
        target: &Path,
        context: &JobContext,
    ) -> io::Result<VerifiedCopy> {
        check(context)?;
        let memory =
            context.memory().claim(super::super::RESERVATION_BYTES).map_err(io::Error::other)?;
        let mut inputs = Vec::new();
        let mut compatibility = super::super::compatibility::Compatibility::default();
        for entry in inventory.entries() {
            let Some(relative) = entry.path().strip_prefix("app/") else {
                continue;
            };
            check(context)?;
            let path = target.join(relative);
            let parent = Directory::open(
                path.parent().ok_or_else(|| invalid("Prepared payload parent absent"))?,
                false,
            )?;
            let mut file = parent.read(
                path.file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| invalid("Prepared payload leaf absent"))?,
            )?;
            if file.metadata()?.len() != entry.bytes()
                || hash(&mut file, context)? != entry.digest()
            {
                return Err(invalid("Prepared application payload verification failed"));
            }
            compatibility.inspect_verified(entry.path(), &mut file)?;
            inputs.push(Input { file, _parent: parent });
        }
        check(context)?;
        compatibility.validate_pair()?;
        Ok(VerifiedCopy { compatibility, _inputs: inputs, _memory: memory })
    }

    /// Require entry support from the actual pinned application's authenticated declaration.
    pub(in crate::setup_storage) fn require_installed_launch(&self) -> io::Result<()> {
        self.compatibility.require_installed_launch()
    }
}
