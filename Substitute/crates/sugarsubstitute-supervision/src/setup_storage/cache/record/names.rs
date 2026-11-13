// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate cache inventories with one bounded canonical index for aliases and ancestry.
use super::super::*;
use std::collections::BTreeMap;

/// Files and empty directories have disjoint canonical names; a file cannot own descendants.
/// Bound canonical Unicode expansion independently of serialized input and allocation counts.
pub(super) fn validate(files: &[String], directories: &[String]) -> io::Result<()> {
    let mut names = BTreeMap::new();
    let mut encoded_bytes = 0usize;
    let mut canonical_bytes = 0usize;
    for (name, file) in
        files.iter().map(|name| (name, true)).chain(directories.iter().map(|name| (name, false)))
    {
        encoded_bytes = encoded_bytes
            .checked_add(name.len().saturating_add(4))
            .ok_or_else(|| invalid("Cache inventory length overflowed"))?;
        if encoded_bytes > MAX_RECORD - 1024 || name.split('/').count() > MAX_DEPTH {
            return Err(invalid("Cache inventory exceeds its name or directory-depth capacity"));
        }
        files::relative(name)
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidInput, cause))?;
        let canonical = name.to_lowercase();
        canonical_bytes = canonical_bytes
            .checked_add(canonical.len())
            .ok_or_else(|| invalid("Cache canonical name length overflowed"))?;
        if canonical_bytes > 2 * MAX_RECORD {
            return Err(invalid("Cache canonical name index exceeds admission"));
        }
        if names.insert(canonical, file).is_some() {
            return Err(invalid("Cache inventory contains aliased names"));
        }
    }
    for name in names.keys() {
        let mut current = name.as_str();
        while let Some((parent, _)) = current.rsplit_once('/') {
            if names.get(parent) == Some(&true) {
                return Err(invalid("Cache inventory aliases files and directories"));
            }
            current = parent;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup_storage::test_worker::{self as worker, TestResult};

    #[test]
    fn large_unicode_inventory_remains_bounded_through_encoding_decoding_and_alias_validation()
    -> TestResult {
        worker::run_reserved(16 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            let lease = RuntimeCache::open(root.path(), CachePolicy::default(), context)?.acquire(
                Namespace::PackedPayloads,
                &admission::storage_key(b"bounded metadata"),
                1,
                100,
                context,
            )?;
            let _workspace = context.memory().claim(RESERVATION_BYTES)?;
            let mut record = lease.record.clone();
            record.files =
                (0..MAX_FILES).map(|index| format!("{index:04x}-{}", "İ".repeat(56))).collect();
            record.validate(record.namespace, &record.key)?;
            let bytes = serde_json::to_vec(&record)?;
            assert!(bytes.len() <= MAX_RECORD);
            let decoded: record::Record = serde_json::from_slice(&bytes)?;
            decoded.validate(decoded.namespace, &decoded.key)?;
            assert_eq!(decoded.files, record.files);
            Ok(())
        })
    }

    #[test]
    fn canonical_index_rejects_unicode_case_duplicates_and_file_directory_ancestry_aliases() {
        assert!(validate(&["İ".into(), "i\u{307}".into()], &[]).is_err());
        assert!(validate(&["File".into()], &["file/child".into()]).is_err());
        assert!(validate(&["directory/child".into()], &["DIRECTORY/CHILD".into()]).is_err());
        assert!(validate(&["directory/child".into()], &["directory/empty".into()]).is_ok());
    }
}
