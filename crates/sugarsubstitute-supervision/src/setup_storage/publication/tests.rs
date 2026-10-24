// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise journal recovery with real Windows file identities, protection and publication.
use super::*;
use crate::durable_storage::protection;
use crate::setup_storage::test_worker::{self as worker, TestResult};

mod preservation;

#[test]
fn successful_publication_remains_successful_when_journal_cleanup_is_busy() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            let mut stage = Stage::create(&parent, protection, context)?;
            stage.file().write_all(b"complete")?;
            let journal = stage.record.journal_name();
            let reader = parent.read(&journal)?;
            stage.publish(&parent.path().join("payload"), false, protection, context)?;
            assert_eq!(std::fs::read(parent.path().join("payload"))?, b"complete");
            assert!(parent.path().join(&journal).exists());
            drop(reader);
            recover(&parent, protection, context)?;
            assert!(!parent.path().join(journal).exists());
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn abandoned_payload_survives_handle_close_and_is_recovered_idempotently() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let (source, journal) = protection::with(context, |protection| {
            let mut stage = Stage::create(&parent, protection, context)?;
            stage.file().write_all(b"partial owned bytes")?;
            let paths = (
                parent.path().join(stage.record.stage()),
                parent.path().join(stage.record.journal_name()),
            );
            drop(stage);
            Ok(paths)
        })?;
        assert_eq!(std::fs::read(&source)?, b"partial owned bytes");
        assert!(journal.exists());
        protection::with(context, |protection| {
            recover(&parent, protection, context)?;
            recover(&parent, protection, context)
        })?;
        assert!(!source.exists());
        assert!(!journal.exists());
        Ok(())
    })
}

#[test]
fn completed_publication_removes_journal_and_keeps_exact_payload() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            write_immutable(&parent, "payload", b"complete", protection, context)?;
            write_immutable(&parent, "payload", b"complete", protection, context)?;
            assert!(write_immutable(&parent, "payload", b"changed", protection, context).is_err());
            recover(&parent, protection, context)
        })?;
        assert_eq!(std::fs::read(parent.path().join("payload"))?, b"complete");
        assert_eq!(std::fs::read_dir(parent.path())?.count(), 1);
        Ok(())
    })
}

#[test]
fn recovery_after_rename_retires_only_journal_never_published_destination() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let destination = parent.path().join("payload");
        protection::with(context, |protection| {
            let mut stage = Stage::create(&parent, protection, context)?;
            stage.file().write_all(b"accepted")?;
            stage.file.sync_all()?;
            protection.close_file(stage.file)?;
            protection.publish(
                &parent.path().join(stage.record.stage()),
                &destination,
                stage.record.file,
                Publication::File { replace: false },
                context,
            )?;
            assert!(parent.path().join(stage.record.journal_name()).exists());
            Ok(())
        })?;
        protection::with(context, |protection| recover(&parent, protection, context))?;
        assert_eq!(std::fs::read(destination)?, b"accepted");
        assert_eq!(std::fs::read_dir(parent.path())?.count(), 1);
        Ok(())
    })
}

#[test]
fn failed_publication_preserves_destination_and_next_pass_cleans_owned_stage() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let destination = parent.path().join("user-file");
        std::fs::write(&destination, b"authored")?;
        protection::with(context, |protection| {
            let mut stage = Stage::create(&parent, protection, context)?;
            stage.file().write_all(b"candidate")?;
            assert!(stage.publish(&destination, false, protection, context).is_err());
            Ok(())
        })?;
        protection::with(context, |protection| recover(&parent, protection, context))?;
        assert_eq!(std::fs::read(destination)?, b"authored");
        assert_eq!(std::fs::read_dir(parent.path())?.count(), 1);
        Ok(())
    })
}

#[test]
fn busy_stage_is_preserved_then_removed_when_external_reader_closes() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            let stage = Stage::create(&parent, protection, context)?;
            let name = stage.record.stage();
            let journal = parent.path().join(stage.record.journal_name());
            drop(stage);
            let reader = parent.read(&name)?;
            recover(&parent, protection, context)?;
            assert!(journal.exists());
            assert!(parent.path().join(&name).exists());
            drop(reader);
            recover(&parent, protection, context)?;
            assert!(!journal.exists());
            assert!(!parent.path().join(name).exists());
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn expired_recovery_preserves_journal_for_another_admitted_operation() -> TestResult {
    worker::run_expiring(|context, clock| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            let stage = Stage::create(&parent, protection, context)?;
            let source = parent.path().join(stage.record.stage());
            let journal = parent.path().join(stage.record.journal_name());
            drop(stage);
            clock.expire();
            assert!(recover(&parent, protection, context).is_err());
            assert!(source.exists());
            assert!(journal.exists());
            Ok(())
        })?;
        Ok(())
    })
}
