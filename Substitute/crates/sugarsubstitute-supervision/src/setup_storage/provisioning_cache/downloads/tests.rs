// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native download custody, whole-grant admission and feature-marker preservation.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, pipeline::Pipeline};
const JOB_BYTES: u64 = 32 * 1024 * 1024;

/// Construct a real exclusive generated grant on the production execution boundary.
fn access(
    root: &std::path::Path,
    bytes: u64,
    context: &JobContext,
) -> io::Result<ProvisioningDownloads> {
    let cache = RuntimeCache::open(root, cache::CachePolicy::default(), context)?;
    let mut lease = cache.acquire(
        Namespace::SetupDownloads,
        &cache::storage_key(b"native download tests"),
        bytes,
        1,
        context,
    )?;
    lease.begin_generated("native downloads fixture", context)?;
    ProvisioningDownloads::new(lease, context)
}

/// Two live share-exclusive files append under one index without reopening each other's handles.
#[test]
fn concurrent_native_payloads_resume_without_rescanning_active_handles() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 16, context)?;
        assert_eq!(access.open("first/payload", Some(7), context)?, 0);
        assert_eq!(access.open("second/payload", Some(9), context)?, 0);
        access.append("first/payload", 0, b"abc", context)?;
        access.append("second/payload", 0, b"123456789", context)?;
        access.validate(context)?;
        assert_eq!(
            access.remove("first/payload", context).err().ok_or("Active deletion accepted")?.kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(access.open("first/payload", None, context).is_err());
        assert!(access.append("first/payload", 0, b"abc", context).is_err());
        access.close("first/payload")?;
        assert_eq!(access.open("first/payload", Some(7), context)?, 3);
        access.append("first/payload", 3, b"defg", context)?;
        access.close("first/payload")?;
        access.close("second/payload")?;
        assert_eq!(access.length("first/payload", context)?, Some(7));
        access.validate(context)?;
        assert_eq!(std::fs::read(access.path()?.join("first/payload"))?, b"abcdefg");
        Ok(())
    })
}

/// Expected extents and original prefixes consume the same original aggregate byte grant.
#[test]
fn whole_grant_pressure_rejects_before_creating_payload_parents_or_changing_bytes() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 8, context)?;
        access.open("payload", Some(8), context)?;
        access.append("payload", 0, b"original", context)?;
        assert_eq!(
            access
                .open("uncreated/file", Some(1), context)
                .err()
                .ok_or("Overload accepted")?
                .kind(),
            io::ErrorKind::StorageFull
        );
        assert!(!access.path()?.join("uncreated").exists());
        assert!(access.append("payload", 8, b"x", context).is_err());
        access.close("payload")?;
        assert_eq!(std::fs::read(access.path()?.join("payload"))?, b"original");
        access.remove("payload", context)?;
        access.open("uncreated/file", Some(1), context)?;
        access.close("uncreated/file")?;
        Ok(())
    })
}

/// A complete replacement includes its simultaneous old target and staged bytes before effects.
#[test]
fn sidecar_replacement_admits_the_complete_stage_and_preserves_original_on_pressure() -> TestResult
{
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 8, context)?;
        access.write("record", b"original", context)?;
        assert_eq!(
            access
                .write("record", b"x", context)
                .err()
                .ok_or("Replacement overload accepted")?
                .kind(),
            io::ErrorKind::StorageFull
        );
        assert_eq!(access.read("record", context)?.as_slice(), b"original");
        assert_eq!(std::fs::read_dir(access.path()?)?.count(), 1);
        access.remove("record", context)?;
        access.write("record", b"new", context)?;
        access.write("record", b"next", context)?;
        assert_eq!(access.read("record", context)?.as_slice(), b"next");
        access.validate(context)?;
        Ok(())
    })
}

/// Native markers preserve the exact persisted schema, Unicode key and UTF-16 identity bound.
#[test]
fn hashed_entries_preserve_schema_one_markers_and_original_upstream_identity() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 65_536, context)?;
        let key = "糖😀\"/upstream";
        let entry = access.entry(key, context)?;
        assert_eq!(entry, access.path()?.join(format!("{:x}", Sha256::digest(key.as_bytes()))));
        let name = format!("{:x}/.substitute-cache.json", Sha256::digest(key.as_bytes()));
        let record = access.read(&name, context)?;
        assert_eq!(
            std::str::from_utf8(record.as_slice())?,
            "{\"schema_version\":1,\"upstreamKey\":\"糖😀\\\"/upstream\"}"
        );
        assert_eq!(access.entry(key, context)?, entry);
        assert!(access.entry("", context).is_err());
        assert!(access.entry(&"😀".repeat(513), context).is_err());
        access.entry(&"😀".repeat(512), context)?;
        Ok(())
    })
}

/// Unknown nonempty directories and changed owner markers never acquire fresh feature authority.
#[test]
fn unmarked_and_changed_download_entries_are_preserved() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 65_536, context)?;
        let directory = format!("{:x}", Sha256::digest(b"upstream"));
        let orphan = format!("{directory}/orphan");
        access.open(&orphan, Some(7), context)?;
        access.append(&orphan, 0, b"unknown", context)?;
        access.close(&orphan)?;
        assert!(access.entry("upstream", context).is_err());
        assert_eq!(access.read(&orphan, context)?.as_slice(), b"unknown");
        let marker = format!("{directory}/.substitute-cache.json");
        access.write(&marker, b"foreign", context)?;
        assert!(access.entry("upstream", context).is_err());
        assert_eq!(access.read(&marker, context)?.as_slice(), b"foreign");
        Ok(())
    })
}

/// Actual held access and original file custody outlive individual job outcomes and block cleanup.
#[test]
fn access_clones_block_finalization_and_retired_writers_flush_their_resumable_prefix() -> TestResult
{
    let mut pipeline = Pipeline::new()?;
    let (root, mut access, clone) = pipeline.run(JOB_BYTES, |context| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 8, &context)?;
        access.open("payload", Some(8), &context)?;
        access.append("payload", 0, b"prefix", &context)?;
        let clone = access.clone();
        Ok((root, access, clone))
    })?;
    pipeline.run(10 * 1024 * 1024, move |context| {
        assert_eq!(
            access.finish(false, &context).err().ok_or("Held grant finalized")?.kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(std::fs::write(access.path()?.join("payload"), b"foreign").is_err());
        drop(clone);
        access.finish(false, &context)?;
        assert_eq!(std::fs::read(access.path()?.join("payload"))?, b"prefix");
        assert!(access.open("payload", None, &context).is_err());
        drop(access);
        root.close()?;
        Ok(())
    })
}

/// Case aliases and relative traversal cannot create another name or bypass the actual root.
#[test]
fn generated_names_reject_aliases_and_unsafe_paths() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 65_536, context)?;
        access.write("entry/Record", b"original", context)?;
        for name in [
            "entry/record",
            "../escape",
            "entry/../escape",
            "C:/escape",
            "entry\\escape",
            "CON",
            "entry./payload",
        ] {
            assert!(access.open(name, None, context).is_err(), "Accepted {name}");
        }
        assert_eq!(access.read("entry/Record", context)?.as_slice(), b"original");
        Ok(())
    })
}

/// A newly introduced second link cannot be reopened or removed by a previously admitted name.
#[test]
fn changed_native_file_links_reject_without_touching_either_name() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let access = access(root.path(), 65_536, context)?;
        access.write("payload", b"original", context)?;
        let file = access.path()?.join("payload");
        let alias = root.path().join("external-alias");
        std::fs::hard_link(&file, &alias)?;
        assert!(access.length("payload", context).is_err());
        assert!(access.read("payload", context).is_err());
        assert!(access.validate(context).is_err());
        assert!(access.open("payload", None, context).is_err());
        assert!(access.remove("payload", context).is_err());
        assert_eq!(std::fs::read(&file)?, b"original");
        assert_eq!(std::fs::read(&alias)?, b"original");
        Ok(())
    })
}

/// Initial discovery rejects a second directory link without taking ownership of its external bytes.
#[test]
fn preexisting_hard_links_reject_native_download_inventory_without_mutation() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), cache::CachePolicy::default(), context)?;
        let mut lease = cache.acquire(
            Namespace::SetupDownloads,
            &cache::storage_key(b"linked download fixture"),
            65_536,
            1,
            context,
        )?;
        lease.begin_generated("linked native fixture", context)?;
        let authored = root.path().join("authored");
        std::fs::write(&authored, b"preserve")?;
        std::fs::hard_link(&authored, lease.path().join("linked"))?;
        assert!(ProvisioningDownloads::new(lease, context).is_err());
        assert_eq!(std::fs::read(authored)?, b"preserve");
        Ok(())
    })
}
