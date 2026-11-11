// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transferred read streams preserve the exact cache entry through native retirement.
use super::*;
use std::io::{Seek, SeekFrom};

#[test]
fn transferred_reader_keeps_entry_pinned_and_seekable_until_the_file_is_dropped() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"retained tile source");
        let cache = open(root.path(), 32, context)?;
        let mut entry = cache.clone().fresh(Namespace::UpdateDownloads, &key, 16, 10, context)?;
        entry.write("tiles", b"0123456789abcdef", context)?;
        entry.publish(context)?;
        let mut reader = entry.into_file_reader("tiles", context)?;
        let now = 10 + Namespace::UpdateDownloads.retention();
        let report = cache.clone().maintain(now, context)?;
        assert_eq!(report.pinned_entries, 1);
        assert_eq!(report.removed_entries, 0);
        reader.seek(SeekFrom::Start(10))?;
        let mut bytes = [0; 6];
        reader.read_exact(&mut bytes)?;
        assert_eq!(&bytes, b"abcdef");
        drop(reader);
        assert_eq!(cache.maintain(now, context)?.removed_entries, 1);
        Ok(())
    })
}

#[test]
fn failed_transferred_reader_releases_its_pin_without_granting_an_undeclared_file() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"undeclared tile source");
        let cache = open(root.path(), 32, context)?;
        let mut entry = cache.clone().fresh(Namespace::UpdateDownloads, &key, 16, 10, context)?;
        entry.write("tiles", b"verified source", context)?;
        entry.publish(context)?;
        drop(entry);
        let entry = cache
            .clone()
            .find(Namespace::UpdateDownloads, &key, 10, context)?
            .ok_or("Published source absent")?;
        assert!(entry.into_file_reader("../authoritative", context).is_err());
        assert_eq!(
            cache.maintain(10 + Namespace::UpdateDownloads.retention(), context)?.removed_entries,
            1
        );
        Ok(())
    })
}
