// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain protected bounded record streams and exact staged publication under native directory pins.
use super::{
    check,
    files::{self, Directory},
    invalid, protection, staging,
};
use std::io::{Read, Write};
use std::{fs::File, io, path::PathBuf};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Retained source handle; the higher-level metadata capability also retains the original lease.
pub(crate) struct StreamRead {
    file: File,
    _parent: Directory,
    remaining: u64,
    failed: bool,
    _memory: MemoryClaim,
}
/// Complete staged replacement with exact byte admission; dropping preserves the original and stage.
pub(crate) struct StreamWrite {
    file: File,
    _parent: Directory,
    staged: PathBuf,
    target: PathBuf,
    remaining: u64,
    failed: bool,
    _memory: MemoryClaim,
}
/// Inert stage identity lets the cache owner admit its exact additional name before creation.
pub(crate) struct StreamStage<'a> {
    parent: &'a Directory,
    name: String,
    staged: PathBuf,
    length: u64,
}
impl StreamRead {
    /// Pin the actual parent and regular single-link source before exposing bounded read operations.
    pub(crate) fn open(
        parent: &Directory,
        name: &str,
        maximum: u64,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        leaf(name)?;
        let memory = context.memory().claim(128 * 1024).map_err(io::Error::other)?;
        let retained = pin(parent)?;
        let file = retained.read(name)?;
        files::single_link(&file)?;
        protection::with(context, |protection| protection.verify_file(&file))?;
        let remaining = file.metadata()?.len();
        if remaining > maximum {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Stored record exceeds its admitted bound",
            ));
        }
        check(context)?;
        Ok(Self { file, _parent: retained, remaining, failed: false, _memory: memory })
    }
}
impl<'a> StreamStage<'a> {
    /// Select a unique sibling stage without creating files or granting unbounded writes.
    pub(crate) fn plan(
        parent: &'a Directory,
        name: &str,
        length: u64,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        leaf(name)?;
        Ok(Self {
            parent,
            name: name.into(),
            staged: staging::unique_path(parent, "record-stream")?,
            length,
        })
    }
    /// Exact additional flat name for feature admission; this remains an observation, not a handle.
    pub(crate) fn stage_name(&self) -> io::Result<&str> {
        self.staged
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Record stage name is absent"))
    }
    /// Create the charged protected stream only after the caller admits its full extent and name.
    pub(crate) fn create(self, context: &JobContext) -> io::Result<StreamWrite> {
        check(context)?;
        let memory = context.memory().claim(128 * 1024).map_err(io::Error::other)?;
        let parent = pin(self.parent)?;
        match parent.read(&self.name) {
            Ok(file) => {
                files::single_link(&file)?;
                protection::with(context, |protection| protection.verify_file(&file))?;
            }
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(cause),
        }
        let file = protection::with(context, |protection| protection.file(&self.staged, context))?;
        Ok(StreamWrite {
            file,
            target: parent.path().join(&self.name),
            _parent: parent,
            staged: self.staged,
            remaining: self.length,
            failed: false,
            _memory: memory,
        })
    }
}
/// Retain an independently owned actual directory instead of borrowing a caller's path lifetime.
fn pin(parent: &Directory) -> io::Result<Directory> {
    let retained = Directory::open(parent.path(), false)?;
    if retained.identity()? != parent.identity()? {
        return Err(invalid("Record stream parent changed"));
    }
    Ok(retained)
}
/// Streams publish a single record in the retained parent, never a nested path or alternate stream.
fn leaf(name: &str) -> io::Result<()> {
    files::relative(name)?;
    if name.contains('/') {
        return Err(invalid("Record stream requires one flat leaf"));
    }
    Ok(())
}
impl StreamRead {
    /// Read at most 8 KiB on the current admitted worker; unexpected shortening is observable.
    pub(crate) fn next(&mut self, context: &JobContext) -> io::Result<Vec<u8>> {
        check(context)?;
        if self.failed {
            return Err(invalid("Record source is no longer readable"));
        }
        let mut bytes = vec![0; self.remaining.min(8192) as usize];
        if let Err(cause) = self.file.read_exact(&mut bytes) {
            self.failed = true;
            return Err(cause);
        }
        self.remaining -= bytes.len() as u64;
        if let Err(cause) = check(context) {
            self.failed = true;
            return Err(cause);
        }
        Ok(bytes)
    }
    /// The original native length, rather than an untrusted wire flag, determines completion.
    pub(crate) fn done(&self) -> bool {
        self.remaining == 0 && !self.failed
    }
}
impl StreamWrite {
    /// Append only the exact admitted extent; callers serialize writes for the whole grant.
    pub(crate) fn append(&mut self, bytes: &[u8], context: &JobContext) -> io::Result<()> {
        if bytes.len() > 8192 {
            return Err(invalid("Record chunk exceeds its admitted extent"));
        }
        StreamOutput { stream: self, context }.write_all(bytes)
    }
    /// Lend sequential bounded writes to a feature callback without exposing file or seek access.
    pub(crate) fn write_with(
        &mut self,
        context: &JobContext,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> io::Result<()> {
        check(context)?;
        let result = write(&mut StreamOutput { stream: self, context });
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    /// Flush all bytes before exact-handle protected publication; incomplete stages remain intact.
    pub(crate) fn commit(self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        if self.failed || self.remaining != 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Record upload incomplete"));
        }
        self.file.sync_all()?;
        let identity = files::file_identity(&self.file)?;
        drop(self.file);
        protection::with(context, |protection| {
            protection.publish(
                &self.staged,
                &self.target,
                identity,
                protection::Publication::File { replace: true },
                context,
            )
        })
    }
}
/// Each actual write updates the original admitted extent, including partial native writes.
struct StreamOutput<'a> {
    stream: &'a mut StreamWrite,
    context: &'a JobContext,
}
impl Write for StreamOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        check(self.context)?;
        if self.stream.failed || bytes.len() as u64 > self.stream.remaining {
            return Err(invalid("Record stream exceeds its admitted extent"));
        }
        let count = match self.stream.file.write(bytes) {
            Ok(count) => count,
            Err(cause) => {
                self.stream.failed = true;
                return Err(cause);
            }
        };
        self.stream.remaining -= count as u64;
        if let Err(cause) = check(self.context) {
            self.stream.failed = true;
            return Err(cause);
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        check(self.context)?;
        let result = self.stream.file.flush().and_then(|()| check(self.context));
        if result.is_err() {
            self.stream.failed = true;
        }
        result
    }
}
