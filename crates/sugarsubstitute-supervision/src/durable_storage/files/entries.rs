// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain admitted directory ancestry and original controls through lazy native enumeration.
use super::Directory;
use crate::durable_storage::check;
use std::{ffi::OsString, fs, io, path::PathBuf};
use sugarsubstitute_contracts::execution::JobContext;

/// A native cursor borrows its pinned parent and the original admitted operation.
pub(crate) struct Entries<'a> {
    native: fs::ReadDir,
    parent: &'a Directory,
    context: &'a JobContext,
    ended: bool,
}
/// Even platforms whose entries retain directory handles cannot detach them from custody.
pub(crate) struct Entry<'a> {
    native: fs::DirEntry,
    _parent: &'a Directory,
    context: &'a JobContext,
}
impl Directory {
    /// Enumerate lazily within the caller's admitted storage workspace and inventory limits.
    /// Every entry borrows this parent and the original job. Names grant no access authority.
    /// Native failures retain their causes; cancellation/deadlines remain observable at EOF.
    pub(crate) fn entries<'a>(&'a self, context: &'a JobContext) -> io::Result<Entries<'a>> {
        let native = effect(context, || fs::read_dir(self.path()))?;
        Ok(Entries { native, parent: self, context, ended: false })
    }
}
impl<'a> Iterator for Entries<'a> {
    type Item = io::Result<Entry<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        match effect(self.context, || self.native.next().transpose()) {
            Ok(Some(native)) => {
                Some(Ok(Entry { native, _parent: self.parent, context: self.context }))
            }
            Ok(None) => {
                self.ended = true;
                None
            }
            Err(cause) => {
                self.ended = true;
                Some(Err(cause))
            }
        }
    }
}
impl std::iter::FusedIterator for Entries<'_> {}

impl Entry<'_> {
    /// Copy the original native name without reopening it or requiring Unicode conversion.
    pub(crate) fn file_name(&self) -> OsString {
        self.native.file_name()
    }
    /// Observe a location while its original parent stays borrowed; the path grants no access.
    pub(crate) fn path(&self) -> PathBuf {
        self.native.path()
    }
    /// Observe the entry kind under original controls without following its final link.
    pub(crate) fn file_type(&self) -> io::Result<fs::FileType> {
        effect(self.context, || self.native.file_type())
    }
    /// Preserve Windows enumeration metadata for a file held by a share-exclusive writer.
    /// This retains the standard entry snapshot semantics rather than reopening the file.
    pub(crate) fn metadata(&self) -> io::Result<fs::Metadata> {
        effect(self.context, || self.native.metadata())
    }
    /// Reobserve current metadata without following the final link, under original controls.
    pub(crate) fn current_metadata(&self) -> io::Result<fs::Metadata> {
        effect(self.context, || fs::symlink_metadata(self.path()))
    }
}

/// Observe controls around an actual native boundary without discarding a simultaneous failure.
fn effect<T>(context: &JobContext, operation: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    check(context)?;
    let result = operation();
    match (result, check(context)) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(source), Err(control)) => {
            Err(io::Error::new(control.kind(), InterruptedEffect { source, control }))
        }
    }
}
/// Retain the original I/O cause and the control that ended the same effect.
#[derive(Debug)]
struct InterruptedEffect {
    source: io::Error,
    control: io::Error,
}
impl std::fmt::Display for InterruptedEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}; native storage effect failed: {}", self.control, self.source)
    }
}
impl std::error::Error for InterruptedEffect {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests;
