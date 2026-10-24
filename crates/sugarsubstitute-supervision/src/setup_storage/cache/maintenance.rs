// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Evict only unpinned recorded files and replay durable cleanup journals after interruption.
use super::*;
mod retirement;
use retirement::journal;
use std::collections::BTreeMap;

/// Observable cleanup outcome. Unknown and pinned content stays intact even under pressure.
#[derive(Default, Debug)]
pub struct Maintenance {
    /// Disk bytes charged to active ceilings and inspected idle payloads.
    pub charged_bytes: u64,
    /// Number of entries removed, including completed interrupted deletions.
    pub removed_entries: usize,
    /// Active leases skipped by cleanup.
    pub pinned_entries: usize,
    /// Durable producer grants requiring feature-owned recovery before reclamation.
    pub unfinished_entries: usize,
    /// Entries whose access advanced after the expiry scan, preserved by final native recheck.
    pub refreshed_entries: usize,
    /// Unrecognized or damaged entries preserved without adoption or deletion.
    pub preserved_entries: usize,
}
/// Keep deterministic eviction facts separate from live native deletion custody.
enum Removal {
    Removed,
    Busy,
    Refreshed,
}

struct Candidate {
    namespace: Namespace,
    key: String,
    touched: u64,
    reserved: u64,
    retiring: bool,
    pinned: bool,
    preserved: bool,
}
impl RuntimeCache {
    /// Apply idle retention and aggregate/namespace disk budgets on an admitted storage worker.
    /// Cross-process pins always win over cleanup. No background work is created.
    /// # Errors
    /// Preserves writer contention, cancellation and unexpected native I/O causes.
    pub fn maintain(&self, now: u64, context: &JobContext) -> io::Result<Maintenance> {
        let _workspace = context.memory().claim(RESERVATION_BYTES).map_err(io::Error::other)?;
        protection::with(context, |protection| {
            let _writer = self.writer(protection, context)?;
            self.reclaim(None, now, context)
        })
        .map_err(record::public_error)
    }
    /// Enforce admission and retention under the already held cache-scope writer.
    pub(super) fn reclaim(
        &self,
        requested: Option<(Namespace, u64)>,
        now: u64,
        context: &JobContext,
    ) -> io::Result<Maintenance> {
        let mut outcome = Maintenance::default();
        for entry in self.scope.root.entries(context)? {
            check(context)?;
            let entry = entry?;
            let name = entry.file_name();
            let known = name.to_str().is_some_and(|name| {
                ["owner", "writer"].contains(&name)
                    || Namespace::ALL.iter().any(|namespace| name == namespace.directory())
            });
            if !known {
                outcome.preserved_entries += 1;
                if outcome.preserved_entries > MAX_ENTRIES {
                    return Err(invalid("Cache scope contains excessive unrecognized content"));
                }
                outcome.charged_bytes = outcome.charged_bytes.saturating_add(tree::scope_charge(
                    &entry.path(),
                    self.scope.policy.total_bytes,
                    context,
                )?);
            }
        }
        let mut candidates = Vec::new();
        let mut budgets = [0_u64; Namespace::ALL.len()];
        let mut occupied = 0usize;
        for namespace in Namespace::ALL {
            let parent = self.namespace(namespace)?;
            let mut entries = BTreeMap::new();
            for entry in parent.entries(context)? {
                check(context)?;
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let key = name
                    .strip_suffix(".retiring.next")
                    .or_else(|| name.strip_suffix(".retiring"))
                    .unwrap_or(&name);
                if !record::valid_entry_key(key) {
                    outcome.preserved_entries += 1;
                    let charge = tree::preserved_charge(&entry.path(), namespace, context)?;
                    outcome.charged_bytes = outcome.charged_bytes.saturating_add(charge);
                    budgets[index(namespace)?] = budgets[index(namespace)?].saturating_add(charge);
                    continue;
                }
                let retiring = name.ends_with(".retiring");
                entries
                    .entry(key.to_owned())
                    .and_modify(|value| *value |= retiring)
                    .or_insert(retiring);
                if entries.len() > MAX_ENTRIES {
                    return Err(invalid("Cache entry scan capacity exceeded"));
                }
            }
            for (key, retiring) in entries {
                occupied += 1;
                if occupied > MAX_ENTRIES {
                    return Err(invalid("Cache entry scan capacity exceeded"));
                }
                let loaded = protection::with(context, |protection| {
                    let mut record = if retiring {
                        journal(&parent, namespace, &key, protection)?
                    } else {
                        record::load(
                            &Directory::open(&parent.path().join(&key), false)?,
                            namespace,
                            &key,
                            protection,
                        )?
                    };
                    let directory = match Directory::open(&parent.path().join(&key), false) {
                        Ok(directory) => directory,
                        Err(cause)
                            if retiring && record::kind(&cause) == io::ErrorKind::NotFound =>
                        {
                            return Ok((record, 0, false, false));
                        }
                        Err(cause) => return Err(cause),
                    };
                    if directory.identity()? != record.identity {
                        return Err(invalid("Cache cleanup native directory identity changed"));
                    }
                    let _exclusion = match record::exclusion(&directory, protection) {
                        Ok(exclusion) => exclusion,
                        Err(cause) if record::busy(&cause) => {
                            let charge = match record::reader(&directory, protection) {
                                Ok(reader) => {
                                    record.touched = record.touched.max(record::last_use(&reader)?);
                                    tree::inspect(&directory, &record, context)?.bytes
                                }
                                Err(cause) if record::busy(&cause) => record.reserved,
                                Err(cause) => return Err(cause),
                            };
                            return Ok((record, charge, true, false));
                        }
                        Err(cause)
                            if retiring && record::kind(&cause) == io::ErrorKind::NotFound =>
                        {
                            let inventory = tree::inspect(&directory, &record, context)?;
                            return Ok((record, inventory.bytes, false, false));
                        }
                        Err(cause) => return Err(cause),
                    };
                    if record.producer.is_some() {
                        let charged = record.reserved.max(tree::preserved_charge(
                            &directory.path().join("data"),
                            namespace,
                            context,
                        )?);
                        return Ok((record, charged, true, false));
                    }
                    record.touched = record.touched.max(record::last_use(&_exclusion)?);
                    match tree::inspect(&directory, &record, context) {
                        Ok(inventory) => Ok((record, inventory.bytes, false, false)),
                        Err(cause) if record::busy(&cause) => {
                            let reserved = record.reserved;
                            Ok((record, reserved, true, false))
                        }
                        Err(cause)
                            if matches!(
                                record::kind(&cause),
                                io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData
                            ) =>
                        {
                            let reserved = record.reserved.max(tree::preserved_charge(
                                directory.path(),
                                namespace,
                                context,
                            )?);
                            tracing::warn!(operation="cache_inventory", namespace=namespace.directory(), entry=%key, cause=%cause, "Preserving unknown cache content");
                            Ok((record, reserved, false, true))
                        }
                        Err(cause) => Err(cause),
                    }
                });
                let (record, mut charge, mut pinned, mut preserved) = match loaded {
                    Ok(loaded) => loaded,
                    Err(cause)
                        if record::kind(&cause) == io::ErrorKind::InvalidInput
                            || record::kind(&cause) == io::ErrorKind::InvalidData
                            || record::kind(&cause) == io::ErrorKind::NotFound =>
                    {
                        outcome.preserved_entries += 1;
                        let charge =
                            tree::preserved_charge(&parent.path().join(&key), namespace, context)?;
                        outcome.charged_bytes = outcome.charged_bytes.saturating_add(charge);
                        budgets[index(namespace)?] =
                            budgets[index(namespace)?].saturating_add(charge);
                        tracing::warn!(operation="cache_scan", namespace=namespace.directory(), entry=%key, cause=%cause, "Preserving unrecognized cache ownership");
                        continue;
                    }
                    Err(cause) => return Err(cause),
                };
                if !pinned && !preserved && record.consumers.is_some() {
                    let path = parent.path().join(&key);
                    let live = match Directory::open(&path, false) {
                        Ok(directory) => consumers::live(&directory, &record, context),
                        Err(cause)
                            if retiring && record::kind(&cause) == io::ErrorKind::NotFound =>
                        {
                            Ok(false)
                        }
                        Err(cause) => Err(cause),
                    };
                    match live {
                        Ok(value) => pinned = value,
                        Err(cause) if record::busy(&cause) => pinned = true,
                        Err(cause)
                            if matches!(
                                record::kind(&cause),
                                io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData
                            ) =>
                        {
                            preserved = true;
                            charge = charge.max(tree::preserved_charge(&path, namespace, context)?);
                            tracing::warn!(operation="cache_native_controls", namespace=namespace.directory(), entry=%key, cause=%cause, "Preserving damaged native consumer controls");
                        }
                        Err(cause) => return Err(cause),
                    }
                }
                outcome.charged_bytes = outcome
                    .charged_bytes
                    .checked_add(charge)
                    .ok_or_else(|| invalid("Cache accounting overflowed"))?;
                let slot = index(namespace)?;
                budgets[slot] = budgets[slot]
                    .checked_add(charge)
                    .ok_or_else(|| invalid("Cache namespace accounting overflowed"))?;
                outcome.pinned_entries += usize::from(pinned);
                outcome.unfinished_entries += usize::from(record.producer.is_some());
                outcome.preserved_entries += usize::from(preserved);
                candidates.push(Candidate {
                    namespace,
                    key,
                    touched: record.touched,
                    reserved: charge,
                    retiring,
                    pinned,
                    preserved,
                });
            }
        }
        candidates.sort_by(|left, right| {
            (!left.retiring, left.touched, &left.key).cmp(&(
                !right.retiring,
                right.touched,
                &right.key,
            ))
        });
        for candidate in candidates {
            let ns = candidate.namespace;
            let needed = requested.map_or(0, |(_, bytes)| bytes);
            let namespace_needed =
                requested.filter(|(requested, _)| *requested == ns).map_or(0, |(_, bytes)| bytes);
            let expired =
                now.checked_sub(candidate.touched).is_some_and(|age| age >= ns.retention());
            if candidate.pinned || candidate.preserved {
                continue;
            }
            let entry_slot_needed =
                requested.is_some_and(|(_, bytes)| bytes > 0) && occupied >= MAX_ENTRIES;
            if !entry_slot_needed
                && !candidate.retiring
                && !expired
                && outcome.charged_bytes.saturating_add(needed) <= self.scope.policy.total_bytes
                && budgets[index(ns)?].saturating_add(namespace_needed) <= ns.budget()
            {
                continue;
            }
            let expiry_only = !entry_slot_needed
                && outcome.charged_bytes.saturating_add(needed) <= self.scope.policy.total_bytes
                && budgets[index(ns)?].saturating_add(namespace_needed) <= ns.budget();
            match self.remove(&candidate, expiry_only.then_some(now), context) {
                Ok(Removal::Removed) => {
                    outcome.removed_entries += 1;
                    occupied -= 1;
                    outcome.charged_bytes -= candidate.reserved;
                    budgets[index(ns)?] -= candidate.reserved;
                }
                Ok(Removal::Busy) => outcome.pinned_entries += 1,
                Ok(Removal::Refreshed) => outcome.refreshed_entries += 1,
                Err(cause) if record::busy(&cause) => outcome.pinned_entries += 1,
                Err(cause)
                    if record::kind(&cause) == io::ErrorKind::InvalidInput
                        || record::kind(&cause) == io::ErrorKind::InvalidData =>
                {
                    outcome.preserved_entries += 1;
                    tracing::warn!(operation="cache_cleanup", namespace=ns.directory(), entry=%candidate.key, cause=%cause, "Preserving unknown cache content");
                }
                Err(cause) => return Err(cause),
            }
        }
        if let Some((namespace, bytes)) = requested
            && (outcome
                .charged_bytes
                .checked_add(bytes)
                .is_none_or(|used| used > self.scope.policy.total_bytes)
                || budgets[index(namespace)?]
                    .checked_add(bytes)
                    .is_none_or(|used| used > namespace.budget())
                || (bytes > 0 && occupied >= MAX_ENTRIES))
        {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "Cache admission blocked by disk budget, pinned entries or unrecognized content",
            ));
        }
        tracing::debug!(
            operation = "cache_maintenance",
            charged_bytes = outcome.charged_bytes,
            removed = outcome.removed_entries,
            pinned = outcome.pinned_entries,
            preserved = outcome.preserved_entries,
            "Runtime cache maintenance completed"
        );
        Ok(outcome)
    }
}
/// Map the exhaustive namespace registry into fixed-size byte counters.
fn index(namespace: Namespace) -> io::Result<usize> {
    Namespace::ALL
        .iter()
        .position(|value| *value == namespace)
        .ok_or_else(|| invalid("Cache namespace is not registered"))
}
