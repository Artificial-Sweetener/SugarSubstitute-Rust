// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real protected payloads and actual execution reservations at the storage owner.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, pipeline::Pipeline};
use std::io::Cursor;

const JOB_BYTES: u64 = 4 * 1024 * 1024;

/// Retain actual file, parent and allocation custody while a later job cannot admit its buffer.
#[test]
fn original_file_parent_and_allocation_survive_jobs_and_reject_unadmitted_restart() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (root, payload) = pipeline.run(JOB_BYTES, |context| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), &context)?;
        payload.receive_exact(0, 7, &mut Cursor::new(b"initial"), &context)?;
        drop(parent);
        Ok((root, payload))
    })?;
    let (root, payload) = pipeline.run(CUSTODY_BYTES, move |context| {
        let mut payload = payload;
        let mut source = Cursor::new(b"changed");
        let failure = payload.receive_exact(0, 7, &mut source, &context)
            .err().ok_or("Unadmitted restart succeeded")?;
        assert!(matches!(failure, TransferFailure::Storage(ref cause)
            if cause.get_ref().and_then(|inner| inner.downcast_ref::<sugarsubstitute_contracts::execution::MemoryClaimError>()).is_some()));
        assert_eq!(source.position(), 0);
        assert_eq!(payload.length()?, 7);
        payload.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        payload.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"initial");
        assert!(std::fs::write(root.path().join("payload"), b"changed").is_err());
        assert!(crate::durable_storage::staging::publish(
            root.path(), &root.path().with_extension("moved")
        ).is_err());
        Ok((root, payload))
    })?;
    pipeline.run(JOB_BYTES, move |context| {
        let mut payload = payload;
        payload.receive_exact(0, 7, &mut Cursor::new(b"changed"), &context)?;
        drop(payload);
        assert_eq!(std::fs::read(root.path().join("payload"))?, b"changed");
        Ok(())
    })
}

/// Reject invalid intervals without consuming input or extending the retained native prefix.
#[test]
fn exact_intervals_preserve_short_prefix_and_reject_holes_overflow_and_extra_source_bytes()
-> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        assert!(matches!(
            payload.receive_exact(0, 7, &mut Cursor::new(b"abc"), context),
            Err(TransferFailure::Length)
        ));
        for (offset, length) in [(4, 2), (2, 5), (3, 5), (u64::MAX, 1)] {
            let mut source = Cursor::new(b"unused");
            assert!(matches!(
                payload.receive_exact(offset, length, &mut source, context),
                Err(TransferFailure::Extent)
            ));
            assert_eq!(source.position(), 0);
            assert_eq!(payload.length()?, 3);
        }
        assert!(matches!(
            payload.receive_exact(3, 4, &mut Cursor::new(b"defgX"), context),
            Err(TransferFailure::Length)
        ));
        assert_eq!(payload.length()?, 3);
        payload.receive_exact(3, 4, &mut Cursor::new(b"defg"), context)?;
        payload.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        payload.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"abcdefg");
        Ok(())
    })
}

/// Keep oversized existing content until its feature explicitly invalidates those exact bytes.
#[test]
fn oversized_existing_bytes_remain_intact_until_explicit_feature_reset() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        payload.receive_exact(0, 7, &mut Cursor::new(b"initial"), context)?;
        drop(payload);
        let mut payload = Payload::open(&parent, "payload", 3, context.memory(), context)?;
        assert_eq!(payload.length()?, 7);
        payload.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        payload.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"initial");
        payload.reset(context)?;
        assert_eq!(payload.length()?, 0);
        payload.receive_exact(0, 3, &mut Cursor::new(b"new"), context)?;
        Ok(())
    })
}

/// Expire current control after a source read and preserve only the earlier durable prefix.
#[test]
fn cancellation_after_source_read_flushes_earlier_prefix_and_forbids_reset() -> TestResult {
    worker::run_reserved_expiring(JOB_BYTES, |context, clock| {
        struct Expire<'a>(&'a worker::TestClock);
        impl Read for Expire<'_> {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                output[0] = b'x';
                self.0.expire();
                Ok(1)
            }
        }
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        assert!(payload.receive_exact(0, 7, &mut Cursor::new(b"abc"), context).is_err());
        assert!(matches!(payload.receive_exact(3, 4, &mut Expire(clock), context),
            Err(TransferFailure::Storage(ref cause)) if cause.kind() == io::ErrorKind::TimedOut));
        assert!(payload.reset(context).is_err());
        drop(payload);
        assert_eq!(std::fs::read(root.path().join("payload"))?, b"abc");
        Ok(())
    })
}

/// Reject unsafe children and hard-linked authoritative bytes before any payload mutation.
#[test]
fn unsafe_names_directories_and_authoritative_hard_links_reject_without_mutation() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        for name in ["../escape", "nested/payload", "payload:stream", "NUL"] {
            assert!(Payload::open(&parent, name, 7, context.memory(), context).is_err());
        }
        assert!(std::fs::read_dir(root.path())?.next().transpose()?.is_none());
        std::fs::create_dir(root.path().join("directory"))?;
        assert!(Payload::open(&parent, "directory", 7, context.memory(), context).is_err());
        let mut payload = Payload::open(&parent, "authoritative", 7, context.memory(), context)?;
        payload.receive_exact(0, 7, &mut Cursor::new(b"initial"), context)?;
        drop(payload);
        std::fs::hard_link(root.path().join("authoritative"), root.path().join("alias"))?;
        assert!(Payload::open(&parent, "alias", 3, context.memory(), context).is_err());
        assert_eq!(std::fs::read(root.path().join("authoritative"))?, b"initial");
        assert_eq!(std::fs::read(root.path().join("alias"))?, b"initial");
        Ok(())
    })
}

/// Use a real read-only native handle to retain both source and durability failures.
#[test]
fn actual_source_and_native_flush_failures_keep_both_original_causes() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        struct Disconnect;
        impl Read for Disconnect {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::ConnectionReset))
            }
        }
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut writer = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        assert!(writer.receive_exact(0, 7, &mut Cursor::new(b"abc"), context).is_err());
        drop(writer);
        // A real read-only native file forces FlushFileBuffers to fail at the external boundary.
        let mut payload = Payload {
            file: parent.read("payload")?,
            _parent: parent.retained()?,
            maximum: 7,
            _memory: context.memory().claim(CUSTODY_BYTES)?,
        };
        let failure = payload
            .receive_exact(3, 4, &mut Disconnect, context)
            .err()
            .ok_or("Native flush failure absent")?;
        let TransferFailure::Flush { transfer, flush } = failure else {
            return Err("Transfer discarded one of its native causes".into());
        };
        assert!(matches!(*transfer, TransferFailure::Storage(ref cause)
            if cause.kind() == io::ErrorKind::ConnectionReset));
        assert_eq!(flush.kind(), io::ErrorKind::PermissionDenied);
        drop(payload);
        assert_eq!(std::fs::read(root.path().join("payload"))?, b"abc");
        Ok(())
    })
}

/// Create, resume and read the same protected file beyond legacy Windows path lengths.
#[test]
fn protected_payload_creation_and_resume_support_long_unicode_parent_paths() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let scope = root.path().join("糖".repeat(80)).join("x".repeat(180));
        std::fs::create_dir_all(&scope)?;
        assert!(scope.join("payload").to_string_lossy().encode_utf16().count() > 260);
        let parent = Directory::open(&scope, false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        assert!(payload.receive_exact(0, 7, &mut Cursor::new(b"abc"), context).is_err());
        drop(payload);
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        payload.receive_exact(3, 4, &mut Cursor::new(b"defg"), context)?;
        payload.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        payload.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"abcdefg");
        Ok(())
    })
}
