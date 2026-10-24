// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Locate compatible generations without repairing damaged ownership or trusting feature content.
use super::*;
use std::collections::BTreeSet;

impl RuntimeCache {
    /// Return the newest structurally valid generation; damaged records are observable misses.
    pub(super) fn latest(
        &self,
        namespace: Namespace,
        semantic: &str,
        writable: bool,
        context: &JobContext,
    ) -> io::Result<Option<(Directory, File, record::Record)>> {
        let parent = self.namespace(namespace)?;
        let prefix = format!("{semantic}-");
        let mut keys = BTreeSet::new();
        for entry in parent.entries(context)? {
            check(context)?;
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if record::valid_entry_key(&name) && name.starts_with(&prefix) {
                keys.insert(name);
            }
            if keys.len() > MAX_ENTRIES {
                return Err(invalid("Cache lookup capacity exceeded"));
            }
        }
        for key in keys.into_iter().rev() {
            let opened = protection::with(context, |protection| {
                let directory = Directory::open(&parent.path().join(&key), false)?;
                // Unpublished replacements must not hide the last complete generation, including
                // while their producer retains its exclusive native lease. Recheck after pinning.
                if record::load(&directory, namespace, &key, protection, context)?.staged {
                    return Ok(None);
                }
                let pin = if writable {
                    record::exclusion(&directory, protection, context)?
                } else {
                    record::reader(&directory, protection, context)?
                };
                let record = record::load(&directory, namespace, &key, protection, context)?;
                if record.staged
                    || record.deleting
                    || (!writable && record.producer.is_some())
                    || files::exists(parent.path().join(format!("{key}.retiring")), context)?
                {
                    return Ok(None);
                }
                consumers::validate(&directory, &record, protection, context)?;
                Ok(Some((directory, pin, record)))
            });
            match opened {
                Ok(Some(entry)) => return Ok(Some(entry)),
                Ok(None) => {}
                Err(cause)
                    if matches!(
                        record::kind(&cause),
                        io::ErrorKind::InvalidInput
                            | io::ErrorKind::InvalidData
                            | io::ErrorKind::NotFound
                    ) =>
                {
                    tracing::warn!(operation="cache_lookup_miss", namespace=namespace.directory(), entry=%key, cause=%cause, "Preserving damaged cache generation");
                }
                Err(cause) => return Err(record::public_error(cause)),
            }
        }
        Ok(None)
    }
    /// Allocate after the highest extant ordinal under the scope writer, including cleanup journals.
    /// Deriving disposable ordering from bounded names avoids a separate authoritative counter.
    pub(super) fn next_key(&self, semantic: &str, context: &JobContext) -> io::Result<String> {
        let mut highest = 0u64;
        let mut keys = BTreeSet::new();
        for namespace in Namespace::ALL {
            let parent = self.namespace(namespace)?;
            for entry in parent.entries(context)? {
                check(context)?;
                let name = entry?.file_name().to_string_lossy().into_owned();
                let key = name
                    .strip_suffix(".retiring.next")
                    .or_else(|| name.strip_suffix(".retiring"))
                    .unwrap_or(&name);
                if record::valid_entry_key(key) {
                    let generation = key
                        .get(65..)
                        .ok_or_else(|| invalid("Cache generation identity changed"))?;
                    highest =
                        highest.max(u64::from_str_radix(generation, 16).map_err(io::Error::other)?);
                    keys.insert((namespace.directory(), key.to_owned()));
                }
                if keys.len() > MAX_ENTRIES {
                    return Err(invalid("Cache generation capacity exceeded"));
                }
            }
        }
        let next =
            highest.checked_add(1).ok_or_else(|| invalid("Cache generation identity exhausted"))?;
        Ok(format!("{semantic}-{next:016x}"))
    }
}
