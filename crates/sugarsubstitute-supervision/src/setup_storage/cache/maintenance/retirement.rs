// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recheck live access and journal exact native deletion custody before reclaiming an entry.
use super::*;

impl RuntimeCache {
    /// Journal intent before deleting only the exact inventoried native objects.
    pub(super) fn remove(
        &self,
        candidate: &Candidate,
        expiry_now: Option<u64>,
        context: &JobContext,
    ) -> io::Result<Removal> {
        let parent = self.namespace(candidate.namespace)?;
        let path = parent.path.join(&candidate.key);
        let directory = match Directory::open(&path, false) {
            Ok(directory) => Some(directory),
            Err(cause) if candidate.retiring && record::kind(&cause) == io::ErrorKind::NotFound => {
                None
            }
            Err(cause) => return Err(cause),
        };
        if let Some(directory) = directory {
            let exclusion = match protection::with(context, |protection| {
                record::exclusion(&directory, protection)
            }) {
                Ok(exclusion) => Some(exclusion),
                Err(cause) if record::busy(&cause) => return Ok(Removal::Busy),
                Err(cause)
                    if candidate.retiring && record::kind(&cause) == io::ErrorKind::NotFound =>
                {
                    None
                }
                Err(cause) => return Err(cause),
            };
            let mut record = protection::with(context, |protection| {
                if candidate.retiring {
                    journal(&parent, candidate.namespace, &candidate.key, protection)
                } else {
                    record::load(&directory, candidate.namespace, &candidate.key, protection)
                }
            })?;
            if directory.identity()? != record.identity {
                return Err(invalid("Cache deletion journal directory changed identity"));
            }
            if record.producer.is_some() || consumers::live(&directory, &record, context)? {
                return Ok(Removal::Busy);
            }
            if !candidate.retiring
                && let Some(now) = expiry_now
            {
                let touched = match &exclusion {
                    Some(file) => record.touched.max(record::last_use(file)?),
                    None => record.touched,
                };
                if now.checked_sub(touched).is_none_or(|age| age < candidate.namespace.retention())
                {
                    return Ok(Removal::Refreshed);
                }
            }
            record.deleting = true;
            let inventory = tree::inspect(&directory, &record, context)?;
            if !candidate.retiring {
                record::start_retirement(&parent, &record, context)?;
            }
            consumers::remove(&directory, &record, context)?;
            for file in inventory.files {
                tree::remove_file(&path.join("data").join(file), context)?;
            }
            tree::remove_directories(&path.join("data"), &record, context)?;
            tree::remove_directory(&path.join("data"), context)?;
            tree::remove_file(&path.join("record.next"), context)?;
            tree::remove_file(&path.join("record"), context)?;
            drop(exclusion);
            tree::remove_file(&path.join("lease"), context)?;
            drop(directory);
            tree::remove_directory(&path, context)?;
        }
        tree::remove_file(&parent.path.join(format!("{}.retiring.next", candidate.key)), context)?;
        tree::remove_file(&parent.path.join(format!("{}.retiring", candidate.key)), context)?;
        Ok(Removal::Removed)
    }
}

/// Recover a bounded protected cleanup journal without relying on missing entry files.
pub(super) fn journal(
    parent: &Directory,
    namespace: Namespace,
    key: &str,
    protection: &Protection,
) -> io::Result<record::Record> {
    let mut file = parent.read(&format!("{key}.retiring"))?;
    protection.verify_file(&file)?;
    let record: record::Record = serde_json::from_slice(&record::read_metadata(&mut file)?)?;
    record.validate(namespace, key)?;
    if !record.deleting {
        return Err(invalid("Cache deletion journal is not retired"));
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self as worker, TestResult};

    #[test]
    fn expiry_rechecks_access_under_exclusion_after_a_reader_releases_its_pin() -> TestResult {
        worker::run_reserved(32 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let key = storage_key(b"late reader");
            let cache =
                || RuntimeCache::open(root.path(), CachePolicy { total_bytes: 16 }, context);
            let mut lease = cache()?.acquire(Namespace::UpdateOffers, &key, 16, 100, context)?;
            lease.write("payload", b"bytes", context)?;
            let path = lease.path();
            let physical = lease.record.key.clone();
            drop(lease);
            let now = 100 + Namespace::UpdateOffers.retention();
            let candidate = Candidate {
                namespace: Namespace::UpdateOffers,
                key: physical,
                touched: 100,
                reserved: 5,
                retiring: false,
                pinned: false,
                preserved: false,
            };
            let reader = cache()?
                .find(Namespace::UpdateOffers, &key, now, context)?
                .ok_or("Missing late reader")?;
            drop(reader);
            assert!(matches!(cache()?.remove(&candidate, Some(now), context)?, Removal::Refreshed));
            assert_eq!(std::fs::read(path.join("payload"))?, b"bytes");
            Ok(())
        })
    }
    #[test]
    fn durable_native_grant_started_after_the_scan_still_blocks_pressure_cleanup() -> TestResult {
        worker::run_reserved(32 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let key = storage_key(b"late native producer");
            let cache =
                || RuntimeCache::open(root.path(), CachePolicy { total_bytes: 64 }, context);
            let mut lease = cache()?.acquire(Namespace::SetupBrowser, &key, 16, 100, context)?;
            lease.write("payload", b"bytes", context)?;
            let candidate = Candidate {
                namespace: Namespace::SetupBrowser,
                key: lease.record.key.clone(),
                touched: 100,
                reserved: 5,
                retiring: false,
                pinned: false,
                preserved: false,
            };
            let path = lease.path();
            lease.begin_generated("feature-native-v1", context)?;
            drop(lease);
            assert!(matches!(cache()?.remove(&candidate, None, context)?, Removal::Busy));
            assert_eq!(std::fs::read(path.join("payload"))?, b"bytes");
            Ok(())
        })
    }
}
