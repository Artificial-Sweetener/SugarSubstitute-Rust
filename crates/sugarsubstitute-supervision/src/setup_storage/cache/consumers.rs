// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain immutable generations through actual native-family use, including host loss.
use super::*;
use crate::setup_storage::cache_custody::CacheFamilyId;
use std::sync::Arc;
mod control;
pub(super) use control::initialize;
const LIMIT: usize = 128;
const MEMORY_BYTES: u64 = 64 * 1024;

/// Native creation owns this reader before creating its private named containment job.
/// The immutable on-disk reference survives loss of the host's native file pins.
pub(crate) struct NativeCachePin {
    family: CacheFamilyId,
    file: Option<File>,
    slot: Directory,
    lease: Arc<CacheLease>,
    identity: (u32, u32, u32),
    retired: bool,
    _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl CacheLease {
    /// Change to a shared immutable pin while holding scope admission against writers and cleanup.
    /// # Errors
    /// Rejects unpublished/native-produced storage, cancellation and native share-mode failures.
    pub fn into_reader(self, context: &JobContext) -> io::Result<Self> {
        check(context)?;
        if self.record.staged || self.record.producer.is_some() {
            return Err(invalid("Cache generation is not published for readers"));
        }
        if !self.writable {
            return Ok(self);
        }
        let cache = self._cache.clone();
        protection::with(context, |protection| {
            let _writer = cache.writer(protection, context)?;
            let CacheLease { directory, _data, record, writable: _, _exclusion, _cache, _memory } =
                self;
            drop(_exclusion);
            let exclusion = record::reader(&directory, protection)?;
            Ok(Self {
                directory,
                _data,
                record,
                writable: false,
                _exclusion: exclusion,
                _cache,
                _memory,
            })
        })
        .map_err(record::public_error)
    }
    /// Bind this immutable source to a unique native family before any process creation effect.
    pub(crate) fn bind_native(
        self: &Arc<Self>,
        family: CacheFamilyId,
        context: &JobContext,
    ) -> io::Result<NativeCachePin> {
        check(context)?;
        if self.writable || self.record.staged || self.record.producer.is_some() {
            return Err(invalid("Native consumers require a published cache reader"));
        }
        let memory = self._cache.scope.memory.claim(MEMORY_BYTES).map_err(io::Error::other)?;
        let bytes = control::reference(self.record.identity, family);
        let name = format!("{}.pin", family.encoded());
        let (slot, file) = protection::with(context, |protection| {
            let _writer = self._cache.writer(protection, context)?;
            let control =
                control::inspect(&self.directory, &self.record, protection, context, false)?
                    .ok_or_else(|| invalid("Cache generation has no native consumer controls"))?;
            let slot = control.directory;
            let mut retained = 0;
            for existing in control.references {
                if existing.family.retired(context)? {
                    match deletion::file(&slot.path.join(&existing.name), existing.identity) {
                        Ok(()) => continue,
                        Err(cause) if record::busy(&cause) => {}
                        Err(cause) if record::kind(&cause) == io::ErrorKind::NotFound => continue,
                        Err(cause) => return Err(cause),
                    }
                }
                retained += usize::from(existing.family != family);
            }
            if retained >= LIMIT {
                return Err(invalid("Cache native consumer capacity exhausted"));
            }
            match slot.read(&name) {
                Ok(mut file) => {
                    protection.verify_file(&file)?;
                    if files::read_bounded(&mut file, control::REFERENCE_BYTES)? != bytes {
                        return Err(invalid("Existing native cache reference changed"));
                    }
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    record::publish(&slot, &name, &bytes, false, context)?;
                }
                Err(cause) => return Err(cause),
            }
            let file = slot.read(&name)?;
            protection.verify_file(&file)?;
            files::single_link(&file)?;
            Ok((slot, file))
        })
        .map_err(record::public_error)?;
        let identity = files::file_identity(&file)?;
        Ok(NativeCachePin {
            family,
            file: Some(file),
            slot,
            lease: self.clone(),
            identity,
            retired: false,
            _memory: memory,
        })
    }
}
impl NativeCachePin {
    /// Release from the registry's unique actual family-settlement result. Supplied exit codes
    /// and cancellation grant no authority; unstarted inputs first become unlaunchable.
    /// Startup cleanup independently requires native object absence.
    pub(crate) fn retire_settled(&mut self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        if self.retired {
            return Ok(());
        }
        let name = format!("{}.pin", self.family.encoded());
        protection::with(context, |protection| {
            let _writer = self.lease._cache.writer(protection, context)?;
            self.file = None;
            match deletion::file(&self.slot.path.join(name), self.identity) {
                Ok(()) => {}
                Err(cause) if record::kind(&cause) == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause),
            }
            self.retired = true;
            Ok(())
        })
        .map_err(record::public_error)
    }
}
/// Validate reader controls before returning cached content. Native lifetime queries are
/// separate: immutable data may be read concurrently with another live native consumer.
pub(super) fn validate(
    directory: &Directory,
    record: &record::Record,
    protection: &Protection,
    context: &JobContext,
) -> io::Result<()> {
    control::inspect(directory, record, protection, context, true).map(|_| ())
}
/// Any existing named family, even empty, keeps immutable bytes live after host loss.
pub(super) fn live(
    directory: &Directory,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<bool> {
    let control = protection::with(context, |protection| {
        control::inspect(directory, record, protection, context, false)
    })?;
    let Some(control) = control else {
        return Ok(false);
    };
    for value in control.references {
        if !value.family.retired(context)? {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Remove exact retired controls only after the entry's deletion journal is durable.
pub(super) fn remove(
    directory: &Directory,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<()> {
    let control = protection::with(context, |protection| {
        control::inspect(directory, record, protection, context, false)
    })?;
    let Some(control) = control else {
        return Ok(());
    };
    for value in &control.references {
        if !value.family.retired(context)? {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "Cache consumer family is live"));
        }
    }
    for value in control.references {
        deletion::file(&control.directory.path.join(value.name), value.identity)?;
    }
    tree::remove_file(&control.directory.path.join("owner"), context)?;
    let path = control.directory.path.clone();
    let identity = control.directory.identity()?;
    drop(control.directory);
    deletion::directory(&path, identity)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native_process::cache_fixture::CacheFamily,
        setup_storage::test_worker::{self as worker, TestResult},
    };
    use std::io::Write;

    /// Own one immutable generation through the same disk and execution admission as production.
    fn source(
        cache: &RuntimeCache,
        key: &str,
        context: &JobContext,
    ) -> io::Result<Arc<CacheLease>> {
        let mut lease = cache.clone().acquire(Namespace::PackedPayloads, key, 8, 100, context)?;
        lease.write("payload", b"bytes", context)?;
        Ok(Arc::new(lease.into_reader(context)?))
    }
    #[test]
    fn native_family_keeps_source_pinned_after_host_file_pins_are_lost() -> TestResult {
        worker::run_reserved(32 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let cache = RuntimeCache::open(root.path(), CachePolicy { total_bytes: 16 }, context)?;
            let key = storage_key(b"native immutable source");
            let source = source(&cache, &key, context)?;
            let path = source.path();
            let mut family = CacheFamily::new(context)?;
            let pin = source.bind_native(family.identity(), context)?;
            drop((pin, source));
            let now = 100 + Namespace::PackedPayloads.retention();
            let report = cache.maintain(now, context)?;
            assert_eq!(report.removed_entries, 0);
            assert_eq!(report.pinned_entries, 1);
            assert!(
                cache.clone().acquire(Namespace::PackedPayloads, &key, 8, now, context).is_err()
            );
            assert_eq!(std::fs::read(path.join("payload"))?, b"bytes");
            family.close()?;
            assert_eq!(cache.maintain(now, context)?.removed_entries, 1);
            assert!(!path.exists());
            Ok(())
        })
    }
    #[test]
    fn interrupted_reference_publication_leaves_a_known_reclaimable_scratch_slot() -> TestResult {
        worker::run_reserved(32 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let cache = RuntimeCache::open(root.path(), CachePolicy { total_bytes: 16 }, context)?;
            let source = source(&cache, &storage_key(b"interrupted native reference"), context)?;
            let path = source.path();
            let family = CacheFamilyId::new(context)?;
            let pending = source
                .directory
                .path
                .join("consumers")
                .join(format!("{}.pin.next", family.encoded()));
            protection::with(context, |protection| {
                let mut file = protection.file(&pending, context)?;
                file.write_all(b"interrupted")?;
                file.sync_all()
            })?;
            drop(source);
            assert_eq!(
                cache
                    .maintain(100 + Namespace::PackedPayloads.retention(), context)?
                    .removed_entries,
                1
            );
            assert!(!path.exists());
            Ok(())
        })
    }
    #[test]
    fn damaged_native_reference_is_a_cache_miss_and_replacement_preserves_its_bytes() -> TestResult
    {
        worker::run_reserved(32 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let cache =
                RuntimeCache::open(root.path(), CachePolicy { total_bytes: 2048 }, context)?;
            let key = storage_key(b"damaged native reference");
            let source = source(&cache, &key, context)?;
            let family = CacheFamilyId::new(context)?;
            let pin = source.bind_native(family, context)?;
            let path = source.path();
            let control =
                source.directory.path.join("consumers").join(format!("{}.pin", family.encoded()));
            drop((pin, source));
            std::fs::write(&control, b"damaged native binding")?;
            assert!(cache.clone().find(Namespace::PackedPayloads, &key, 100, context)?.is_none());
            let next = cache.clone().fresh(Namespace::PackedPayloads, &key, 8, 100, context)?;
            assert_ne!(next.path(), path);
            assert_eq!(std::fs::read(control)?, b"damaged native binding");
            assert_eq!(std::fs::read(path.join("payload"))?, b"bytes");
            Ok(())
        })
    }
    #[test]
    fn oversized_native_scratch_is_preserved_and_charged_before_new_admission() -> TestResult {
        worker::run_reserved(32 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let cache = RuntimeCache::open(root.path(), CachePolicy { total_bytes: 64 }, context)?;
            let source = source(&cache, &storage_key(b"oversized native control"), context)?;
            let family = CacheFamilyId::new(context)?;
            let path = source
                .directory
                .path
                .join("consumers")
                .join(format!("{}.pin.next", family.encoded()));
            protection::with(context, |protection| {
                let mut file = protection.file(&path, context)?;
                file.write_all(&[3; control::REFERENCE_BYTES + 1])?;
                file.sync_all()
            })?;
            drop(source);
            let report = cache.maintain(100, context)?;
            assert_eq!(report.preserved_entries, 1);
            assert!(report.charged_bytes > 64);
            assert!(
                cache
                    .clone()
                    .fresh(Namespace::PackedPayloads, &storage_key(b"new source"), 8, 100, context)
                    .is_err()
            );
            assert_eq!(std::fs::read(path)?, [3; control::REFERENCE_BYTES + 1]);
            Ok(())
        })
    }
}
