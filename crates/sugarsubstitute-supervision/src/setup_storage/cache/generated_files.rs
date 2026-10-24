// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep aggregate generated-file admission authoritative while native payload handles remain open.
mod index;
mod payloads;
mod records;
use super::*;
use crate::durable_storage::payload::Payload;
use index::Index;
use std::collections::BTreeMap;
use sugarsubstitute_contracts::execution::MemoryClaim;

/// One original generated grant and its admitted names, bytes and exclusive native writers.
/// The feature retains this owner through actual process-family retirement. A current job funds
/// traversal/protection work; the original cache reservation funds the persistent index and files.
pub(in crate::setup_storage) struct GeneratedFiles {
    payloads: BTreeMap<String, Payload>,
    index: Index,
    lease: CacheLease,
    _memory: MemoryClaim,
}
impl GeneratedFiles {
    /// Inventory once before opening any share-exclusive payload; later admission uses this index.
    pub(in crate::setup_storage) fn new(
        lease: CacheLease,
        context: &JobContext,
    ) -> io::Result<Self> {
        lease.require_writable()?;
        if lease.producer().is_none() {
            return Err(invalid("Generated-file producer is absent"));
        }
        let memory = lease._cache.scope.memory.claim(4 * 1024 * 1024).map_err(io::Error::other)?;
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let physical = tree::generated_records(&lease.directory, context)?;
        let mut index = Index::new(lease.record.reserved, lease.path())?;
        for name in physical.directories {
            index.admit(&name, true, 0)?;
        }
        for name in physical.files {
            check(context)?;
            let path = lease.path().join(&name);
            let parent = Directory::open(
                path.parent().ok_or_else(|| invalid("File parent absent"))?,
                false,
            )?;
            let file = parent.read(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| invalid("File name absent"))?,
            )?;
            files::single_link(&file)?;
            index.admit(&name, false, file.metadata()?.len())?;
        }
        Ok(Self { payloads: BTreeMap::new(), index, lease, _memory: memory })
    }

    /// Borrow only the observed location; it is not a writable file or a fresh grant.
    pub(in crate::setup_storage) fn path(&self) -> PathBuf {
        self.lease.path()
    }

    /// Preserve existing ancestry and never follow an exchanged directory or reparse point.
    fn parent(&self, name: &str, create: bool, context: &JobContext) -> io::Result<Directory> {
        let mut parent = self.lease._data.retained()?;
        let mut parts = name.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                break;
            }
            check(context)?;
            let path = parent.path().join(part);
            let existing = if create { None } else { Some(Directory::open(&path, false)?) };
            protection::with(context, |protection| {
                protection.directory(
                    &path,
                    if create { Mode::OpenOrCreate } else { Mode::Existing },
                    context,
                )?;
                Ok(())
            })?;
            parent = match existing {
                Some(directory) => directory,
                None => Directory::open(&path, false)?,
            };
        }
        Ok(parent)
    }

    /// Require the same exclusive producer on every ordinary effect.
    fn check(&self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.lease.require_writable()?;
        if self.lease.producer().is_none() {
            return Err(invalid("Generated-file producer retired"));
        }
        Ok(())
    }

    /// Inspect generated contents through retained writers without releasing queued-byte admission.
    pub(in crate::setup_storage) fn validate(&self, context: &JobContext) -> io::Result<()> {
        self.check(context)?;
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let actual = tree::generated_open_records(&self.lease.directory, &self.payloads, context)?;
        self.index.validate(&actual)
    }

    /// Only the feature's actual family-retirement path may call this after access clones retire.
    pub(in crate::setup_storage) fn record(&mut self, context: &JobContext) -> io::Result<()> {
        while let Some(name) = self.payloads.keys().next().cloned() {
            self.close(&name)?;
        }
        if self.lease.producer().is_some() {
            self.lease.record_generated(utc_seconds()?, context)?;
        }
        Ok(())
    }
    /// Retire a feature-selected disposable grant after its actual native family has settled.
    pub(in crate::setup_storage) fn retire(&mut self, context: &JobContext) -> io::Result<()> {
        self.lease.retire(context)
    }
}
