// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native metadata admission, byte custody and preservation on the actual execution owner.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, pipeline::Pipeline};

fn access(
    root: &std::path::Path,
    bytes: u64,
    context: &JobContext,
) -> io::Result<ProvisioningMetadata> {
    let cache = RuntimeCache::open(root, cache::CachePolicy::default(), context)?;
    let mut lease = cache.acquire(
        Namespace::UpstreamMetadata,
        &cache::storage_key(b"metadata tests"),
        bytes,
        1,
        context,
    )?;
    lease.begin_generated("native metadata fixture", context)?;
    Ok(ProvisioningMetadata::new(lease))
}
fn write(access: &ProvisioningMetadata, bytes: &[u8], context: &JobContext) -> io::Result<()> {
    let mut writer = access.write(MetadataRecord::Fetch, bytes.len() as u64, context)?;
    for chunk in bytes.chunks(8192) {
        writer.append(chunk, context)?;
    }
    writer.commit(context)
}
#[test]
fn original_grant_and_streams_survive_actual_job_retirement() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (root, access, writer) = pipeline.run(32 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 64 * 1024, &context)?;
        let writer = access.write(MetadataRecord::Fetch, 16_384, &context)?;
        Ok((root, access, writer))
    })?;
    let (root, mut access, mut reader) = pipeline.run(10 * 1024 * 1024, move |context| {
        let mut writer = writer;
        assert_eq!(
            access.read(MetadataRecord::Fetch, &context).err().ok_or("Overlap accepted")?.kind(),
            io::ErrorKind::WouldBlock
        );
        writer.append(&vec![b'a'; 8192], &context)?;
        writer.append(&vec![b'b'; 8192], &context)?;
        writer.commit(&context)?;
        let reader = access.read(MetadataRecord::Fetch, &context)?;
        Ok((root, access, reader))
    })?;
    pipeline.run(10 * 1024 * 1024, move |context| {
        assert!(access.finish(false, &context).is_err());
        assert_eq!(reader.next(&context)?.as_slice(), vec![b'a'; 8192]);
        assert!(!reader.done());
        assert_eq!(reader.next(&context)?.as_slice(), vec![b'b'; 8192]);
        assert!(reader.done());
        drop(reader);
        access.finish(false, &context)?;
        assert!(access.write(MetadataRecord::Fetch, 1, &context).is_err());
        drop(access);
        root.close()?;
        Ok(())
    })?;
    Ok(())
}
#[test]
fn full_replacement_admission_precedes_effects_and_abandoned_stages_stay_charged() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 16, context)?;
        write(&access, b"original", context)?;
        let before = std::fs::read_dir(access.path()?)?.count();
        assert_eq!(
            access.write(MetadataRecord::Fetch, 9, context).err().ok_or("Pressure ignored")?.kind(),
            io::ErrorKind::StorageFull
        );
        assert_eq!(std::fs::read_dir(access.path()?)?.count(), before);
        let mut writer = access.write(MetadataRecord::Fetch, 8, context)?;
        writer.append(b"staged", context)?;
        assert!(writer.commit(context).is_err());
        assert_eq!(std::fs::read(access.path()?.join("fetch-cache.json"))?, b"original");
        assert_eq!(
            access
                .write(MetadataRecord::Fetch, 3, context)
                .err()
                .ok_or("Stage not charged")?
                .kind(),
            io::ErrorKind::StorageFull
        );
        assert_eq!(std::fs::read_dir(access.path()?)?.count(), before + 1);
        Ok(())
    })
}
#[test]
fn unknown_names_hard_links_and_empty_records_preserve_authoritative_bytes() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 1024, context)?;
        for name in [
            "../fetch-cache.json",
            "settings.json",
            "fetch-cache.json:stream",
            "nested/fetch-cache.json",
        ] {
            assert!(serde_json::from_value::<MetadataRecord>(serde_json::json!(name)).is_err());
        }
        write(&access, b"", context)?;
        let mut reader = access.read(MetadataRecord::Fetch, context)?;
        assert!(reader.next(context)?.as_slice().is_empty());
        assert!(reader.done());
        drop(reader);
        let authored = root.path().join("authored");
        std::fs::write(&authored, b"preserve")?;
        std::fs::hard_link(&authored, access.path()?.join("torch-stack-cache.json"))?;
        assert!(access.read(MetadataRecord::TorchStack, context).is_err());
        assert!(access.write(MetadataRecord::TorchStack, 1, context).is_err());
        assert_eq!(std::fs::read(authored)?, b"preserve");
        Ok(())
    })
}
#[test]
fn expired_context_cannot_publish_a_completed_stage_or_create_another() -> TestResult {
    worker::run_reserved_expiring(32 * 1024 * 1024, |context, clock| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 1024, context)?;
        write(&access, b"original", context)?;
        let mut writer = access.write(MetadataRecord::Fetch, 3, context)?;
        writer.append(b"new", context)?;
        clock.expire();
        assert!(writer.commit(context).is_err());
        assert!(access.write(MetadataRecord::Fetch, 1, context).is_err());
        assert_eq!(std::fs::read(access.path()?.join("fetch-cache.json"))?, b"original");
        Ok(())
    })
}
/// Exercise both real inventory bounds with fixture I/O separated from owner admission checks.
#[test]
fn legacy_inventory_limits_reject_new_stages_without_effects_or_cleanup() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    for (count, prefix, expected) in [
        (8192, "item".to_owned(), "Metadata staging inventory is full"),
        (3000, "a".repeat(200), "Metadata inventory names exceed their bound"),
    ] {
        let (root, access, directory) = pipeline.run(32 * 1024 * 1024, |context| {
            let root = tempfile::tempdir()?;
            let access = access(root.path(), 1024, &context)?;
            let directory = access.path()?;
            Ok((root, access, directory))
        })?;
        inventory_files(&mut pipeline, &directory, &prefix, count, |file| {
            std::fs::write(file, b"")
        })?;
        let (root, access, directory) = pipeline.run(10 * 1024 * 1024, move |context| {
            let cause = access
                .write(MetadataRecord::Fetch, 1, &context)
                .err()
                .ok_or("Metadata stage exceeded its inventory admission")?;
            assert_eq!(cause.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(cause.to_string(), expected);
            let actual = std::fs::read_dir(&directory)?.try_fold(0, |count, entry| {
                entry?;
                Ok::<_, io::Error>(count + 1)
            })?;
            assert_eq!(actual, count);
            Ok((root, access, directory))
        })?;
        inventory_files(&mut pipeline, &directory, &prefix, count, |file| {
            std::fs::remove_file(file)
        })?;
        pipeline.run(1024 * 1024, move |_| {
            drop(access);
            root.close()?;
            Ok(())
        })?;
    }
    Ok(())
}

/// Bound fixture construction and disposal separately from the actual owner assertion.
/// Every filesystem effect remains on admitted workers under the unchanged observer limit.
fn inventory_files(
    pipeline: &mut Pipeline,
    directory: &std::path::Path,
    prefix: &str,
    count: usize,
    effect: fn(&std::path::Path) -> io::Result<()>,
) -> TestResult {
    const FILES_PER_JOB: usize = 256;
    for start in (0..count).step_by(FILES_PER_JOB) {
        let directory = directory.to_owned();
        let prefix = prefix.to_owned();
        pipeline.run(1024 * 1024, move |_| {
            for index in start..(start + FILES_PER_JOB).min(count) {
                effect(&directory.join(format!("{prefix}-{index:04}")))?;
            }
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn protected_metadata_streams_publish_replace_and_read_beyond_legacy_win32_path_lengths()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let scope = root.path().join(format!("long-location-{}", "x".repeat(80)));
        std::fs::create_dir(&scope)?;
        let access = access(&scope, 65_536, context)?;
        assert!(
            access.path()?.join("fetch-cache.json").to_string_lossy().encode_utf16().count() > 260
        );
        write(&access, b"original", context)?;
        let expected = "糖😀".repeat(4000).into_bytes();
        write(&access, &expected, context)?;
        let mut reader = access.read(MetadataRecord::Fetch, context)?;
        let mut actual = Vec::new();
        while !reader.done() {
            actual.extend_from_slice(reader.next(context)?.as_slice());
        }
        assert_eq!(actual, expected);
        Ok(())
    })
}
