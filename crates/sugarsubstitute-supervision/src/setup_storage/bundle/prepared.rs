// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify installation copies completely and admit runtime files independently of diagnostics.
use super::super::{check, files::Directory, invalid};
use super::payload::{Input, hash};
use std::{io, path::Path};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_update::package::Manifest;

/// Verified runtime bytes and resolved ancestors retained against write/replacement.
/// Symbol files are authenticated by the diagnostic owner before symbol consumption.
pub(in crate::setup_storage) struct VerifiedRuntime {
    pub(in crate::setup_storage) compatibility: super::super::compatibility::Compatibility,
    _inputs: Vec<Input>,
    _memory: MemoryClaim,
}

impl VerifiedRuntime {
    /// Borrow every verified payload file and its resolved ancestry for native custody transfer.
    /// The receiver duplicates these exact objects; reopening their paths would lose the pins.
    pub(in crate::setup_storage) fn launch_pins(&self) -> impl Iterator<Item = &std::fs::File> {
        self._inputs
            .iter()
            .flat_map(|input| std::iter::once(&input.file).chain(&input._parent._pins))
    }

    /// Admit runtime files from an already prepared, authenticated generation.
    /// Known diagnostic files do not participate in launch admission, even when absent or damaged.
    pub(in crate::setup_storage) fn open(
        inventory: &Manifest,
        target: &Path,
        context: &JobContext,
    ) -> io::Result<Self> {
        Self::verify(inventory, target, context, Scope::Runtime)
    }

    /// Verify every copied application file before publishing completed preparation.
    /// Existing package custody authenticates the manifest; these handles pin its destination.
    pub(in crate::setup_storage) fn verify_copy(
        inventory: &Manifest,
        target: &Path,
        context: &JobContext,
    ) -> io::Result<Self> {
        Self::verify(inventory, target, context, Scope::CompleteCopy)
    }

    fn verify(
        inventory: &Manifest,
        target: &Path,
        context: &JobContext,
        scope: Scope,
    ) -> io::Result<Self> {
        check(context)?;
        let memory =
            context.memory().claim(super::super::RESERVATION_BYTES).map_err(io::Error::other)?;
        Self::verify_admitted(inventory, target, context, scope, memory)
    }

    /// Reuse the inventory reader's storage reservation for runtime pins without doubling custody.
    pub(super) fn open_admitted(
        inventory: &Manifest,
        target: &Path,
        context: &JobContext,
        memory: MemoryClaim,
    ) -> io::Result<Self> {
        Self::verify_admitted(inventory, target, context, Scope::Runtime, memory)
    }

    /// The caller retains one complete storage reservation throughout inventory and file admission.
    fn verify_admitted(
        inventory: &Manifest,
        target: &Path,
        context: &JobContext,
        scope: Scope,
        memory: MemoryClaim,
    ) -> io::Result<Self> {
        let mut inputs = Vec::new();
        let mut compatibility = super::super::compatibility::Compatibility::default();
        for entry in inventory.entries() {
            let Some(relative) = entry.path().strip_prefix("app/") else {
                continue;
            };
            if matches!(scope, Scope::Runtime)
                && super::super::inventory::is_diagnostic_file(relative)
            {
                continue;
            }
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
        Ok(Self { compatibility, _inputs: inputs, _memory: memory })
    }

    /// Require entry support from the actual pinned application's authenticated declaration.
    pub(in crate::setup_storage) fn require_installed_launch(&self) -> io::Result<()> {
        self.compatibility.require_installed_launch()
    }

    /// Installed engines require protected execution inputs as well as their retained manifest.
    pub(super) fn require_protected(
        &self,
        protection: &super::super::protection::Protection,
    ) -> io::Result<()> {
        for input in &self._inputs {
            protection.verify_file(&input.file)?;
        }
        Ok(())
    }
}

/// Copy completion and execution admission have different required inputs.
enum Scope {
    CompleteCopy,
    Runtime,
}

#[cfg(test)]
mod tests;
