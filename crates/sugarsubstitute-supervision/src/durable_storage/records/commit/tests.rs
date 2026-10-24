// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual prepared publication after deadline expiry and bounded-byte rejection.
use super::*;
use crate::{
    durable_storage::protection,
    setup_storage::test_worker::{self as worker, TestResult},
};

/// An admitted prepared commit publishes accepted progress after its original deadline expires.
#[test]
fn prepared_commit_completes_after_expiry_without_acquiring_another_job_or_grant() -> TestResult {
    worker::run_reserved_expiring(2 * 1024 * 1024, |context, clock| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        protection::with(context, |protection| {
            let commit =
                PreparedCommit::new(&parent, "record", "pending", 32, protection, context)?;
            clock.expire();
            commit.commit(b"accepted security progress")?;
            assert_eq!(
                check(context)
                    .err()
                    .ok_or_else(|| io::Error::other("Deadline remained live"))?
                    .kind(),
                io::ErrorKind::TimedOut
            );
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path().join("record"))?, b"accepted security progress");
        Ok(())
    })
}

/// Bad bounds never acquire a stage; invalid encoded bytes preserve the authoritative record.
#[test]
fn prepared_commit_rejects_invalid_bounds_and_preserves_existing_bytes() -> TestResult {
    worker::run_reserved(2 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        std::fs::write(root.path().join("record"), b"original")?;
        protection::with(context, |protection| {
            assert!(
                PreparedCommit::new(&parent, "record", "pending", 0, protection, context).is_err()
            );
            for bytes in [b"".as_slice(), b"oversized"] {
                let commit =
                    PreparedCommit::new(&parent, "record", "pending", 4, protection, context)?;
                assert!(commit.commit(bytes).is_err());
                assert_eq!(std::fs::read(root.path().join("record"))?, b"original");
            }
            Ok(())
        })?;
        Ok(())
    })
}
