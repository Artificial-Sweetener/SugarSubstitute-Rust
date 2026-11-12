// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read an actual completed inert diagnostic copy without opening its old active namespace.
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
};
use crate::{
    diagnostic_storage::{
        DIAGNOSTIC_IO_BYTES, DIAGNOSTIC_SCAN_BOUND, DiagnosticEntryClass, DiagnosticObject,
    },
    durable_storage::{
        check,
        files::{self, Directory},
        imports, invalid,
        protected::record_file,
        protection,
    },
    setup_storage::RunningApplication,
};
use std::{
    io::{self, Read, Seek, SeekFrom},
    os::windows::io::IntoRawHandle,
};
use sugarsubstitute_contracts::{
    execution::JobContext, incident::MAX_PAYLOAD_BYTES, lifecycle::NamespaceId,
};
use windows::Win32::Foundation::HANDLE;

/// Original source retention; the crash owner separately admits its bounded decoded history.
pub const PRESERVED_DIAGNOSTIC_SOURCE_BYTES: u64 = imports::WORKSPACE_BYTES;

/// Session-local physical matching data; it grants no path, file or active-store authority.
#[derive(Clone, Copy)]
pub struct PreservedDiagnosticEntry {
    ordinal: u16,
    class: DiagnosticEntryClass,
    bytes: u64,
}
impl PreservedDiagnosticEntry {
    /// Observe the fixed ordinal, valid only while this actual source remains retained.
    pub const fn ordinal(self) -> u16 {
        self.ordinal
    }
    /// Observe the existing native classifier's candidate category before codec validation.
    pub const fn class(self) -> DiagnosticEntryClass {
        self.class
    }
    /// Observe this pinned regular file's exact byte length.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}
struct RetainedFile {
    name: String,
    file: Handle,
    identity: [u8; 24],
}

/// Original completed receipt and actual immutable copy pins belong to one finite read operation.
/// Later history/export requests open a fresh session; this owner cannot renew its original job.
pub struct PreservedDiagnosticSource {
    entries: Vec<PreservedDiagnosticEntry>,
    files: Vec<RetainedFile>,
    tree: Option<imports::PreservedTree>,
    tree_sha256: [u8; 32],
    retiring: Vec<Handle>,
    retired: bool,
    _budget: Budget,
    namespace: NamespaceId,
    context: JobContext,
}
impl PreservedDiagnosticSource {
    /// Open a fixed passive namespace below the actual retained running installation's user root.
    /// The existing copy receipt must bind logical/native installation and destination identities,
    /// and its complete current tree must still match the immutable copied-content digest.
    /// This performs no claim enrollment, mkdir, permission repair, journal replay or activation.
    /// # Errors
    /// Preserves missing/foreign/corrupt receipt, changed copy, links, protection, bound and original
    /// control causes. Empty/oversize/unknown filesystem shapes remain intact and unavailable.
    pub fn open(
        running: &RunningApplication,
        namespace: NamespaceId,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        if context.deadline().is_none() {
            return Err(invalid("Preserved diagnostics require a finite original job"));
        }
        let layout = sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
            running.directory(),
        );
        let root = Directory::open(layout.root(), false)?.resolved()?;
        Self::open_at(
            &root,
            imports::ImportAssociation::Installed(running.installation_id()),
            namespace,
            context,
        )
    }
    pub(in crate::setup_storage::user_data) fn open_at(
        root: &Directory,
        association: imports::ImportAssociation<'_>,
        namespace: NamespaceId,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        if context.deadline().is_none() {
            return Err(invalid("Preserved diagnostics require a finite original job"));
        }
        let leaf = record_file::hex(namespace.bytes());
        let target = root.path().join("user/migration-originals/diagnostic-namespaces").join(leaf);
        let tree = imports::retain_preserved_tree(root, association, &target, context)?;
        Self::from_tree(tree, namespace, context)
    }
    fn from_tree(
        tree: imports::PreservedTree,
        namespace: NamespaceId,
        context: &JobContext,
    ) -> io::Result<Self> {
        let mut entries = Vec::new();
        let mut files = Vec::new();
        let mut retiring = Vec::new();
        let pin_count = tree
            .retirement_pins()?
            .checked_add(1)
            .ok_or_else(|| invalid("Preserved rendezvous pin admission overflow"))?;
        let retained = pin_count
            .checked_mul(size_of::<Handle>())
            .and_then(|bytes| {
                bytes.checked_add(
                    DIAGNOSTIC_SCAN_BOUND
                        * (size_of::<RetainedFile>()
                            + size_of::<PreservedDiagnosticEntry>()
                            + 1024),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(4 * 1024 * 1024 + 8192 * size_of::<std::fs::File>() + 512 * 1024)
            })
            .ok_or_else(|| invalid("Preserved diagnostic retention admission overflow"))?;
        if retained as u64 > imports::WORKSPACE_BYTES {
            return Err(invalid("Preserved diagnostic retention exceeds its original admission"));
        }
        retiring.try_reserve_exact(pin_count).map_err(io::Error::other)?;
        let budget = tree.budget();
        entries.try_reserve_exact(DIAGNOSTIC_SCAN_BOUND).map_err(io::Error::other)?;
        files.try_reserve_exact(DIAGNOSTIC_SCAN_BOUND).map_err(io::Error::other)?;
        protection::with(context, |owner| {
            let mut visited = 0;
            for entry in tree.directory().entries(context)? {
                let entry = entry?;
                visited += 1;
                if visited > DIAGNOSTIC_SCAN_BOUND {
                    return Err(invalid("Preserved diagnostic inventory exceeds admission"));
                }
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid("Preserved diagnostic name is not Unicode"))?;
                if name.len() > 1024 {
                    return Err(invalid("Preserved diagnostic name exceeds admission"));
                }
                let file = tree.directory().read(&name)?;
                files::single_link(&file)?;
                owner.verify_file(&file)?;
                let bytes = files::length(&file, context)?;
                // The old empty rendezvous remains in the full inert tree. It is never an
                // acquired writer or a semantic incident record in this read-only projection.
                if name == record_file::DIAGNOSTIC_WRITER_LEAF {
                    if bytes != 0 {
                        return Err(invalid("Preserved diagnostic rendezvous is not empty"));
                    }
                    retiring.push(Handle::new(HANDLE(file.into_raw_handle()), budget.clone()));
                    continue;
                }
                if bytes == 0 || bytes > MAX_PAYLOAD_BYTES {
                    return Err(invalid("Preserved diagnostic object exceeds admission"));
                }
                let ordinal = u16::try_from(entries.len()).map_err(io::Error::other)?;
                entries.push(PreservedDiagnosticEntry {
                    ordinal,
                    class: record_file::diagnostic_entry_class(&name),
                    bytes,
                });
                files.push(RetainedFile {
                    name,
                    identity: files::native_identity(&file)?,
                    file: Handle::new(HANDLE(file.into_raw_handle()), budget.clone()),
                });
            }
            check(context)
        })?;
        let tree_sha256 = tree.sha256();
        Ok(Self {
            entries,
            files,
            tree: Some(tree),
            tree_sha256,
            retiring,
            retired: false,
            _budget: budget,
            namespace,
            context: context.clone(),
        })
    }
    /// Borrow complete bounded candidate observations; incident codecs stay with their owner.
    pub fn entries(&self) -> &[PreservedDiagnosticEntry] {
        &self.entries
    }
    /// Observe verified original attribution without granting old-namespace native permissions.
    pub const fn namespace(&self) -> NamespaceId {
        self.namespace
    }
    /// Observe the fully verified inert copy's content commitment.
    pub fn tree_sha256(&self) -> [u8; 32] {
        self.tree_sha256
    }
    /// Read one bounded chunk through its actual pinned ordinary file, with exact EOF semantics.
    /// The source's retained original operation supplies every cancellation and deadline check;
    /// no caller-supplied context can stand in for it.
    /// # Errors
    /// Rejects an ended original operation, changed identity/length, invalid ordinal/offset or a
    /// buffer above64KiB.
    pub fn read_chunk(
        &mut self,
        ordinal: u16,
        offset: u64,
        output: &mut [u8],
    ) -> io::Result<usize> {
        self.check()?;
        if output.len() > DIAGNOSTIC_IO_BYTES {
            return Err(invalid("Preserved diagnostic chunk exceeds admission"));
        }
        let expected = self
            .entries
            .get(usize::from(ordinal))
            .ok_or_else(|| invalid("Preserved diagnostic ordinal is unknown"))?;
        let entry = self
            .files
            .get_mut(usize::from(ordinal))
            .ok_or_else(|| invalid("Preserved diagnostic pin is absent"))?;
        let file = entry.file.borrow_file().map_err(io::Error::other)?;
        if offset > expected.bytes
            || files::length(&file, &self.context)? != expected.bytes
            || files::native_identity(&file)? != entry.identity
        {
            return Err(invalid("Preserved diagnostic source changed"));
        }
        let mut input = &*file;
        input.seek(SeekFrom::Start(offset))?;
        check(&self.context)?;
        let count = input.read(output)?;
        check(&self.context)?;
        Ok(count)
    }
    /// Bind a decoded logical object using the existing canonical native leaf owner.
    /// # Errors
    /// Preserves original controls, invalid ordinals and canonical-name construction failures.
    pub fn matches(&self, ordinal: u16, object: DiagnosticObject) -> io::Result<bool> {
        self.check()?;
        if object.namespace().is_some_and(|namespace| namespace != self.namespace) {
            return Ok(false);
        }
        let entry = self
            .files
            .get(usize::from(ordinal))
            .ok_or_else(|| invalid("Preserved diagnostic ordinal is unknown"))?;
        let expected = record_file::DiagnosticLeaf::object(object, self.namespace)
            .map_err(io::Error::other)?;
        Ok(expected.text().map_err(io::Error::other)? == entry.name)
    }
    /// Revalidate the complete immutable copied generation under its original operation.
    /// # Errors
    /// Any changed inventory/content/native identity or ended original operation refuses.
    pub fn revalidate(&self) -> io::Result<()> {
        self.check()?;
        self.tree
            .as_ref()
            .ok_or_else(|| invalid("Preserved diagnostic source retirement began"))?
            .revalidate(&self.context)
    }
    /// Check-close only this source's already held read pins inside an admitted finite cleanup.
    /// Original source memory funds these cells; no old writer or new read can be admitted here.
    /// A failed close retains the exact physical handle and its original accounting for retry.
    /// # Errors
    /// Rejects another executor or missing/expired cleanup deadline, and preserves OS close causes.
    pub fn retire(&mut self, cleanup: &JobContext) -> io::Result<()> {
        if cleanup.owner_id() != self.context.owner_id()
            || cleanup.deadline().is_none_or(|until| cleanup.now() >= until)
        {
            return Err(invalid("Preserved source cleanup differs from its actual finite owner"));
        }
        if let Some(tree) = self.tree.take() {
            tree.begin_retirement(&mut self.retiring);
        }
        let mut outcome = Ok(());
        for file in &mut self.files {
            outcome = ClaimError::finish(outcome, file.file.close());
        }
        for pin in &mut self.retiring {
            outcome = ClaimError::finish(outcome, pin.close());
        }
        if outcome.is_ok() {
            self.retired = true;
            self.files.clear();
            self.retiring.clear();
        }
        outcome.map_err(io::Error::other)
    }
    /// Observe successful checked closure independently of the source job's terminal delivery.
    pub const fn retired(&self) -> bool {
        self.retired
    }
    fn check(&self) -> io::Result<()> {
        if self.tree.is_none() || self.retired {
            return Err(invalid("Preserved diagnostic source retirement began"));
        }
        check(&self.context)
    }
}

#[cfg(test)]
mod tests;
