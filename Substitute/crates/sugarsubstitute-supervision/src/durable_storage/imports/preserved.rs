// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a completed inert copy's exact content generation without granting activation authority.
use super::*;
use crate::native_claims::files::{Budget, Handle};
use std::os::windows::io::IntoRawHandle;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::MemoryClaim;
use windows::Win32::Foundation::HANDLE;

/// Existing receipt/journal and immutable native copy pins outlive every borrowed passive read.
pub(crate) struct PreservedTree {
    directory: Directory,
    observed: tree::Snapshot,
    _records: Directory,
    _receipt: File,
    _journal: File,
    memory: Arc<MemoryClaim>,
}
impl PreservedTree {
    pub(crate) fn directory(&self) -> &Directory {
        &self.directory
    }
    pub(crate) fn sha256(&self) -> [u8; 32] {
        self.observed.tree.sha256
    }
    pub(crate) fn budget(&self) -> Budget {
        Budget::Protected(self.memory.clone())
    }
    pub(crate) fn retirement_pins(&self) -> io::Result<usize> {
        self.observed
            .pins
            .len()
            .checked_add(self.directory.native_pins().count())
            .and_then(|count| count.checked_add(self._records.native_pins().count()))
            .and_then(|count| count.checked_add(2))
            .ok_or_else(|| invalid("Preserved source pin count overflow"))
    }
    /// Adopt every original pin into the caller's already reserved checked-close cells.
    pub(crate) fn begin_retirement(self, handles: &mut Vec<Handle>) {
        let budget = self.budget();
        for file in self
            .observed
            .pins
            .into_iter()
            .chain(self.directory.into_retirement_pins())
            .chain(self._records.into_retirement_pins())
            .chain([self._receipt, self._journal])
        {
            handles.push(Handle::new(HANDLE(file.into_raw_handle()), budget.clone()));
        }
    }
    /// Compare the complete still-pinned copy generation, never its originals' physical IDs.
    pub(crate) fn revalidate(&self, context: &JobContext) -> io::Result<()> {
        let current = tree::observe(&self.directory, RootEntries::Except(&[]), context)?;
        if current.tree != self.observed.tree {
            return Err(invalid("Preserved copy generation changed"));
        }
        check(context)
    }
}

/// Verify the actual complete copied tree through existing import codecs and retain read custody.
/// This existing-only port never creates journals, repairs protection, activates or replays IDs.
pub(crate) fn retain_preserved_tree(
    installation: &Directory,
    association: ImportAssociation<'_>,
    destination: &Path,
    context: &JobContext,
) -> io::Result<PreservedTree> {
    check(context)?;
    let memory = context.memory().claim(WORKSPACE_BYTES).map_err(io::Error::other)?;
    let (parent, destination) = user_paths::parent(installation, destination, context)?;
    let records = records(installation, false, context)?;
    let (receipt, receipt_file) = read_receipt(&records, &receipt_name(&destination)?, context)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Preserved copy receipt absent"))?;
    let journal = read_journal(&records, &receipt, context)?;
    let directory = Directory::open(&destination, false)?.resolved()?;
    protection::with(context, |owner| {
        owner.directory(
            directory.path(),
            super::super::protected::directories::Mode::Existing,
            context,
        )?;
        Ok(())
    })?;
    if !receipt.journal.belongs_to(
        association,
        installation.identity()?,
        parent.identity()?,
        &destination,
    ) || directory.identity()? != receipt.stage_identity
    {
        return Err(invalid("Preserved copy does not match its original completed association"));
    }
    let observed = tree::observe(&directory, RootEntries::Except(&[]), context)?;
    if !observed.tree.same_contents(&receipt.tree) {
        return Err(invalid("Preserved copy bytes differ from its immutable import receipt"));
    }
    check(context)?;
    Ok(PreservedTree {
        directory,
        observed,
        _records: records,
        _receipt: receipt_file,
        _journal: journal,
        memory: Arc::new(memory),
    })
}
