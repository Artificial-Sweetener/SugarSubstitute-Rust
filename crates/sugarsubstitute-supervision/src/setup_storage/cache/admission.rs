// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Charge every retained entry before allowing a producer to grow its disposable payload.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;

/// Hash a feature-owned semantic identity into a safe storage component.
pub fn storage_key(identity: &[u8]) -> String {
    format!("{:x}", Sha256::digest(identity))
}

impl RuntimeCache {
    /// Find an existing compatible storage entry, retaining exclusion before reading its record.
    /// Feature owners must revalidate bytes before reuse; absence returns `None`.
    /// # Errors
    /// Reports busy consumers, cancellation and native I/O; damaged or unfinished entries are misses.
    pub fn find(
        self,
        namespace: Namespace,
        key: &str,
        now: u64,
        context: &JobContext,
    ) -> io::Result<Option<CacheLease>> {
        check(context)?;
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let memory = self.scope.memory.claim(ENTRY_BYTES).map_err(io::Error::other)?;
        if !record::valid_key(key) {
            return Err(invalid("Cache key is invalid"));
        }
        let Some((directory, exclusion, mut record)) =
            self.latest(namespace, key, false, context)?
        else {
            return Ok(None);
        };
        record::touch(&exclusion, now)?;
        record.touched = record.touched.max(record::last_use(&exclusion)?);
        let data = Directory::open(&directory.path.join("data"), false)?;
        Ok(Some(CacheLease {
            directory,
            record,
            writable: false,
            _data: data,
            _exclusion: exclusion,
            _cache: self,
            _memory: memory,
        }))
    }

    /// Reserve a maximum payload size and retain its native pin before returning a writable path.
    /// Idle owned entries are reclaimed as needed. Existing entries retain their original ceiling;
    /// changing semantic size requires a feature key change. Failed admission preserves pinned data.
    /// # Errors
    /// Reports storage pressure when active/unknown content prevents admission, and preserves
    /// cancellation, ownership, competing writer and native storage failures.
    pub fn acquire(
        self,
        namespace: Namespace,
        key: &str,
        maximum_bytes: u64,
        now: u64,
        context: &JobContext,
    ) -> io::Result<CacheLease> {
        self.admit(namespace, key, maximum_bytes, now, false, context)
    }
    /// Create an independent generation for complete authenticated replacement bytes.
    /// Call `CacheLease::publish` after completing feature verification. Earlier generations retain
    /// their pins and remain visible until the replacement is published.
    /// # Errors
    /// Preserves capacity, cancellation, competing writers and native storage failures.
    pub fn fresh(
        self,
        namespace: Namespace,
        key: &str,
        maximum_bytes: u64,
        now: u64,
        context: &JobContext,
    ) -> io::Result<CacheLease> {
        self.admit(namespace, key, maximum_bytes, now, true, context)
    }
    /// Serialize capacity, native entry acquisition and last-use publication.
    fn admit(
        self,
        namespace: Namespace,
        key: &str,
        maximum: u64,
        now: u64,
        fresh: bool,
        context: &JobContext,
    ) -> io::Result<CacheLease> {
        check(context)?;
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let retained = self.scope.memory.claim(ENTRY_BYTES).map_err(io::Error::other)?;
        if !record::valid_key(key)
            || maximum == 0
            || maximum > namespace.budget()
            || maximum > self.scope.policy.total_bytes
        {
            return Err(invalid("Cache key or byte reservation is invalid"));
        }
        let result = protection::with(context, |protection| {
            let _writer = self.writer(protection, context)?;
            let existing = if fresh { None } else { self.latest(namespace, key, true, context)? };
            let (directory, exclusion, mut record) = match existing {
                Some(existing) => existing,
                None => {
                    let physical = self.next_key(key, context)?;
                    self.reclaim(Some((namespace, maximum)), now, context)?;
                    let parent = self.namespace(namespace)?;
                    let directory =
                        create(&parent, namespace, &physical, maximum, now, fresh, context)?;
                    let exclusion = record::exclusion(&directory, protection)?;
                    let record = record::load(&directory, namespace, &physical, protection)?;
                    (directory, exclusion, record)
                }
            };
            if record.producer.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Cache producer requires observed native retirement recovery",
                ));
            }
            if maximum != record.reserved {
                return Err(invalid("Cache reservation differs from its semantic identity"));
            }
            self.reclaim(Some((namespace, 0)), now, context)?;
            record::touch(&exclusion, now)?;
            record.touched = record.touched.max(record::last_use(&exclusion)?);
            let data = Directory::open(&directory.path.join("data"), false)?;
            Ok((directory, data, exclusion, record))
        })
        .map_err(record::public_error)?;
        let (directory, data, exclusion, record) = result;
        Ok(CacheLease {
            directory,
            record,
            writable: true,
            _data: data,
            _exclusion: exclusion,
            _cache: self,
            _memory: retained,
        })
    }
}
/// Publish complete empty entries before another process can acquire them.
fn create(
    parent: &Directory,
    namespace: Namespace,
    key: &str,
    reserved: u64,
    now: u64,
    unpublished: bool,
    context: &JobContext,
) -> io::Result<Directory> {
    let staged = staging::unique_path(parent, "entry-initializing")?;
    let identity = protection::with(context, |protection| {
        protection.directory(&staged, Mode::CreateNew, context)?;
        let directory = Directory::open(&staged, false)?;
        protection.directory(&staged.join("data"), Mode::CreateNew, context)?;
        let lease = protection.file(&staged.join("lease"), context)?;
        record::initialize_time(&lease, now)?;
        lease.sync_all()?;
        let record = record::Record {
            schema: 1,
            identity: directory.identity()?,
            namespace,
            key: key.to_owned(),
            reserved,
            touched: now,
            deleting: false,
            staged: unpublished,
            producer: None,
            files: Vec::new(),
            directories: Vec::new(),
        };
        let mut file = protection.file(&staged.join("record"), context)?;
        file.write_all(&serde_json::to_vec(&record)?)?;
        file.sync_all()?;
        directory.identity()
    })?;
    let path = parent.path.join(key);
    protection::with(context, |protection| {
        protection.publish(&staged, &path, identity, protection::Publication::Directory, context)
    })?;
    Directory::open(&path, false)
}
