// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise reusable original custody through real cache and image workers.

use super::*;

#[test]
fn retained_encoded_original_rebuilds_the_same_image_and_survives_each_clone() -> harness::TestResult
{
    harness::run_storage(64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let bytes = encoded(&[255, 0, 0, 0, 255, 0])?;
        let cache = RuntimeCache::open(
            &root.path().join("cache"),
            CachePolicy { total_bytes: 1024 * 1024 },
            context,
        )?;
        let mut entry = cache.clone().fresh(
            Namespace::ImageSources,
            &storage_key(b"retained original"),
            bytes.len() as u64,
            10,
            context,
        )?;
        entry.write("output.png", &bytes, context)?;
        entry.publish(context)?;
        let original_path = entry.path().join("output.png");
        let mut input = Some(entry.into_file_reader("output.png", context)?);

        // Exhaust the exact real reservation before attempting the transfer.
        let available = context
            .memory()
            .claim(context.memory().bytes() + 1)
            .err()
            .ok_or(ImportError::Invalid("Expected memory admission failure"))?
            .available_bytes;
        let held = context.memory().claim(available)?;
        assert!(ImageSource::encoded(&mut input, context.memory()).is_err());
        assert!(input.is_some());
        drop(held);

        let source = ImageSource::encoded(&mut input, context.memory())?;
        assert!(input.is_none());
        assert!(source.requires_worker_retirement());
        let other = source.clone();
        let mut first = source.import_image(
            cache.clone(),
            CacheReuse::Compatible,
            context.memory(),
            10,
            context,
        )?;
        let mut rebuilt = other.import_image(
            cache.clone(),
            CacheReuse::Rebuild,
            context.memory(),
            11,
            context,
        )?;
        assert_eq!(first.descriptor().asset(), rebuilt.descriptor().asset());
        assert_eq!(pixels(&mut first, context)?, pixels(&mut rebuilt, context)?);
        let writer = std::fs::OpenOptions::new().write(true).open(&original_path);
        assert!(writer.is_err());
        drop((source, first, rebuilt));
        let retained = cache.clone().maintain(11 + Namespace::ImageSources.retention(), context)?;
        assert_eq!(retained.pinned_entries, 1);
        drop(other);
        assert!(
            cache.maintain(11 + Namespace::ImageSources.retention(), context)?.removed_entries >= 1
        );
        assert!(!ImageSource::file(root.path().join("local.png")).requires_worker_retirement());
        Ok(())
    })
}
