// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain an entry through producer writes and every native/read consumer's actual retirement.
use super::*;

/// Cross-process entry custody. Producers are exclusive; `find` acquires shared read-only pins.
/// Dropping releases its pin and performs no cleanup.
/// Keep this owner through native family settlement, not merely a cancellation request.
pub struct CacheLease {
    pub(super) directory: Directory,
    pub(super) _data: Directory,
    pub(super) record: record::Record,
    pub(super) writable: bool,
    pub(super) _exclusion: File,
    pub(super) _cache: RuntimeCache,
    pub(super) _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl CacheLease {
    /// Pinned content directory. Readers may pass it only to read-only native consumers.
    /// Producers write only declared disposable files or their exclusively granted generated output.
    pub fn path(&self) -> PathBuf {
        self.directory.path().join("data")
    }
    /// Maximum aggregate payload bytes charged before this entry was returned.
    pub fn reserved_bytes(&self) -> u64 {
        self.record.reserved
    }
    /// Declare exact feature-owned relative file names before creating their contents.
    /// Existing names remain declared so interruption never loses cleanup ownership.
    /// # Errors
    /// Rejects aliases, unsafe paths, excessive inventories, cancellation or publication failure.
    pub fn declare(
        &mut self,
        names: impl IntoIterator<Item = String>,
        context: &JobContext,
    ) -> io::Result<()> {
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        self.declare_owned(names, context)
    }
    /// Publish declarations using the caller's already admitted traversal workspace.
    pub(super) fn declare_owned(
        &mut self,
        names: impl IntoIterator<Item = String>,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        self.require_declared_writer()?;
        let mut next = self.record.clone();
        let mut declared: std::collections::BTreeSet<_> = next.files.into_iter().collect();
        for name in names {
            if declared.len() + next.directories.len() == MAX_FILES && !declared.contains(&name) {
                return Err(invalid("Cache file inventory is full"));
            }
            declared.insert(name.into_boxed_str().into_string());
        }
        next.files = declared.into_iter().collect();
        next.validate(next.namespace, &next.key)?;
        record::save(&self.directory, &next, context)?;
        self.record = next;
        Ok(())
    }
    /// Atomically publish one complete file and preserve declared scratch after interruption.
    /// This owner checks total physical payload length against its admitted reservation.
    /// # Errors
    /// Rejects unknown files, capacity exhaustion, cancellation, changed protection or native I/O.
    pub fn write(&mut self, name: &str, bytes: &[u8], context: &JobContext) -> io::Result<()> {
        self.write_with(name, bytes.len() as u64, context, |output| output.write_all(bytes))
    }
    /// Publish an exact bounded stream without buffering its complete content in memory.
    /// The borrowed writer grants sequential writes only and cannot outlive this lease.
    /// Feature owners authenticate content; this owner enforces the declared length and disk grant.
    /// # Errors
    /// Callback and length failures preserve the previous target. Publication/metadata failures
    /// retain complete target bytes and declared scratch; their original causes remain visible.
    pub fn write_with(
        &mut self,
        name: &str,
        length: u64,
        context: &JobContext,
        write: impl FnOnce(&mut dyn std::io::Write) -> io::Result<()>,
    ) -> io::Result<()> {
        super::stream::write(self, name, length, context, write).map_err(record::public_error)
    }
    /// Publish a completed replacement for subsequent lookup after feature-owned verification.
    /// The caller retains its exclusive lease until return and must then release it for readers.
    /// # Errors
    /// Preserves incomplete storage, unsettled native producers, cancellation and metadata failures.
    pub fn publish(&mut self, context: &JobContext) -> io::Result<()> {
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        self.require_declared_writer()?;
        let inventory = self.inspect(context)?;
        if inventory.bytes > self.record.reserved {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Cache publication exceeds admission",
            ));
        }
        let mut next = self.record.clone();
        next.staged = false;
        record::save(&self.directory, &next, context)?;
        self.record = next;
        Ok(())
    }
    /// Record actual feature use with an injected Unix UTC timestamp. Clock regression is clamped.
    /// # Errors
    /// Preserves cancellation and failed metadata publication without releasing custody.
    pub fn touch(&mut self, now: u64, context: &JobContext) -> io::Result<()> {
        check(context)?;
        record::touch(&self._exclusion, now, context)?;
        self.record.touched = self.record.touched.max(record::last_use(&self._exclusion, context)?);
        Ok(())
    }
    /// Open a declared regular cache file; feature owners must verify its content before use.
    /// # Errors
    /// Returns absence distinctly and rejects undeclared paths, links or native read failures.
    pub fn read(&self, name: &str, context: &JobContext) -> io::Result<CacheReader<'_>> {
        self.open_reader(name, context).map(|file| CacheReader::new(file, self))
    }
    /// Transfer this exact entry pin into one regular read-only stream.
    /// The file is closed before its entry pin is released. Move the stream between admitted
    /// jobs instead of reopening/protecting the same tile archive for every region request.
    /// Buffer allocation and cancellation checks remain with the consuming feature worker.
    /// # Errors
    /// Preserves declared-file validation, native opening, protection and cancellation failures.
    pub fn into_file_reader(
        self,
        name: &str,
        context: &JobContext,
    ) -> io::Result<CacheReader<'static>> {
        let lease = self.into_reader(context)?;
        let file = lease.open_reader(name, context)?;
        Ok(CacheReader::owned(file, lease))
    }
    /// Keep borrowed and transferred read custody subject to identical native validation.
    fn open_reader(&self, name: &str, context: &JobContext) -> io::Result<File> {
        if !self.record.files.iter().any(|declared| declared == name) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Cache file is not declared"));
        }
        files::relative(name)
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidInput, cause))?;
        let path = self.path().join(name);
        let parent =
            Directory::open(path.parent().ok_or_else(|| invalid("Cache parent absent"))?, false)?;
        check(context)?;
        let file = parent.read(
            path.file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid("Cache leaf absent"))?,
        )?;
        protection::with(context, |protection| protection.verify_file(&file))?;
        Ok(file)
    }
    /// Validate declared ownership and report physical payload usage without authenticating content.
    /// # Errors
    /// Rejects links, unknown children, oversize output, cancellation and native inspection errors.
    pub fn verify_storage(&self, context: &JobContext) -> io::Result<u64> {
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let tree = self.inspect(context)?;
        if tree.bytes > self.record.reserved {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Cache producer exceeded its byte reservation",
            ));
        }
        Ok(tree.bytes)
    }
    /// Seal this producer's private output after all writing and native consumers have retired.
    /// The lease remains pinned. Later undeclared files prevent cleanup. Feature owners still
    /// authenticate all content on reuse. Authoritative files must never enter this directory.
    /// # Errors
    /// Rejects links, excessive inventories, cancellation and native I/O. Oversized output is
    /// inventoried before reporting storage pressure so idle cleanup can reclaim it safely.
    pub fn record_generated(&mut self, now: u64, context: &JobContext) -> io::Result<u64> {
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        check(context)?;
        self.require_writable()?;
        if self.record.producer.is_none() {
            return Err(invalid("Cache has no pending native producer"));
        }
        let inventory = tree::generated(&self.directory, context)?;
        let mut next = self.record.clone();
        next.producer = None;
        next.files = inventory.files.clone();
        next.directories = inventory.directories.clone();
        next.files.sort();
        next.touched = next.touched.max(now);
        record::save(&self.directory, &next, context)?;
        self.record = next;
        if inventory.bytes > self.record.reserved {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Native cache producer exceeded its admitted disk capacity",
            ));
        }
        Ok(inventory.bytes)
    }
    /// Keep read-only cache pins unable to mutate declared files or inventories.
    pub(super) fn require_writable(&self) -> io::Result<()> {
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Cache reader cannot mutate storage",
            ));
        }
        Ok(())
    }
    /// Keep declared-file publication separate from unsettled opaque native output.
    pub(super) fn require_declared_writer(&self) -> io::Result<()> {
        self.require_writable()?;
        if self.record.producer.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Native cache producer has not retired",
            ));
        }
        Ok(())
    }
    /// Observe exact declared storage while the original entry exclusion is retained.
    pub(super) fn inspect(&self, context: &JobContext) -> io::Result<tree::Inventory> {
        tree::inspect(&self.directory, &self.record, context)
    }
}
