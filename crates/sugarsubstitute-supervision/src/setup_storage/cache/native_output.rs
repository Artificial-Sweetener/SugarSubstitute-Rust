// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Persist opaque producer custody until its feature owner observes actual native retirement.
use super::*;
use std::collections::BTreeSet;

impl CacheLease {
    /// Persist bounded feature-owned recovery facts before exposing this path to a native writer.
    /// The durable pin survives host loss. The feature owns the descriptor schema and must retain
    /// this lease until actual native retirement; cancellation or dropping a future is insufficient.
    /// # Errors
    /// Rejects read-only access, a previous unsettled grant, invalid descriptors and storage failure.
    pub fn begin_generated(&mut self, recovery: &str, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.require_writable()?;
        if self.record.producer.is_some()
            || recovery.is_empty()
            || recovery.len() > 8192
            || recovery.contains('\0')
        {
            return Err(invalid(
                "Native cache producer recovery descriptor is invalid or already active",
            ));
        }
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let mut next = self.record.clone();
        next.producer = Some(recovery.to_owned());
        record::save(&self.directory, &next, context)?;
        self.record = next;
        Ok(())
    }
    /// Borrow matching facts for the feature's retirement recovery; bytes grant no native authority.
    pub fn producer(&self) -> Option<&str> {
        self.record.producer.as_deref()
    }
}
/// Bounded snapshot of pending native grants; callers release each lease before advancing.
/// A live grant cannot prevent inspection of later recoverable entries in the same namespace.
pub struct PendingEntries {
    cache: RuntimeCache,
    namespace: Namespace,
    keys: std::collections::btree_set::IntoIter<String>,
    _memory: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl RuntimeCache {
    /// Enumerate a bounded snapshot for feature-owned native retirement recovery.
    /// Consume this cursor within the current storage job; it retains that job's small index claim.
    /// # Errors
    /// Preserves cancellation and invalid or unexpectedly large native directory inventories.
    pub fn pending(
        &self,
        namespace: Namespace,
        context: &JobContext,
    ) -> io::Result<PendingEntries> {
        check(context)?;
        let memory = context.memory().claim(256 * 1024).map_err(io::Error::other)?;
        let parent = self.namespace(namespace)?;
        let mut keys = BTreeSet::new();
        for entry in parent.entries(context)? {
            check(context)?;
            let name = entry?.file_name().to_string_lossy().into_owned();
            if record::valid_entry_key(&name) {
                keys.insert(name);
            }
            if keys.len() > MAX_ENTRIES {
                return Err(invalid("Native cache recovery capacity exceeded"));
            }
        }
        Ok(PendingEntries {
            cache: self.clone(),
            namespace,
            keys: keys.into_iter(),
            _memory: memory,
        })
    }
}
impl PendingEntries {
    /// Exclusively retain the next unfinished generation and skip live native file pins.
    /// Inspect the feature descriptor and prove actual native retirement before recording output.
    /// Dropping a returned lease preserves its durable producer grant and advances this cursor.
    /// # Errors
    /// Preserves cancellation and unexpected native storage failures; damaged records are misses.
    pub fn next(&mut self, context: &JobContext) -> io::Result<Option<CacheLease>> {
        check(context)?;
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        let memory = self.cache.scope.memory.claim(ENTRY_BYTES).map_err(io::Error::other)?;
        let namespace = self.namespace;
        let parent = self.cache.namespace(namespace)?;
        for key in self.keys.by_ref() {
            let opened = protection::with(context, |protection| {
                let directory = Directory::open(&parent.path().join(&key), false)?;
                let exclusion = record::exclusion(&directory, protection, context)?;
                let record = record::load(&directory, namespace, &key, protection, context)?;
                if record.producer.is_none()
                    || record.deleting
                    || parent.path().join(format!("{key}.retiring")).exists()
                {
                    return Ok(None);
                }
                let data = Directory::open(&directory.path().join("data"), false)?;
                Ok(Some((directory, data, exclusion, record)))
            });
            match opened {
                Ok(Some((directory, data, exclusion, record))) => {
                    return Ok(Some(CacheLease {
                        directory,
                        _data: data,
                        record,
                        writable: true,
                        _exclusion: exclusion,
                        _cache: self.cache.clone(),
                        _memory: memory,
                    }));
                }
                Ok(None) => {}
                Err(cause) if record::busy(&cause) => {}
                Err(cause)
                    if matches!(
                        record::kind(&cause),
                        io::ErrorKind::InvalidInput
                            | io::ErrorKind::InvalidData
                            | io::ErrorKind::NotFound
                    ) =>
                {
                    tracing::warn!(operation="cache_recovery_miss", namespace=namespace.directory(),
                        entry=%key, cause=%cause, "Preserving damaged producer recovery data");
                }
                Err(cause) => return Err(record::public_error(cause)),
            }
        }
        Ok(None)
    }
}
