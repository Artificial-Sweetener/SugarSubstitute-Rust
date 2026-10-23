// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Safe reclamation preserves authoritative and unknown bytes through interrupted deletion.
use super::*;

#[test]
fn unknown_content_and_authoritative_siblings_survive_cleanup_and_pressure() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        for name in ["settings", "signing-trust", "rollback", "user-document"] {
            std::fs::write(root.path().join(name), b"authored")?;
        }
        let mut lease = open(root.path(), 8, context)?.acquire(
            Namespace::UpdateOffers,
            &storage_key(b"route"),
            8,
            100,
            context,
        )?;
        lease.write("signed", b"bytes", context)?;
        std::fs::write(lease.path().join("unknown"), b"preserve")?;
        let path = lease.path();
        drop(lease);
        let report = open(root.path(), 8, context)?
            .maintain(100 + Namespace::UpdateOffers.retention(), context)?;
        assert_eq!(report.preserved_entries, 1);
        assert_eq!(report.removed_entries, 0);
        assert!(
            open(root.path(), 8, context)?
                .acquire(Namespace::UpdateOffers, &storage_key(b"other"), 1, 100, context)
                .is_err()
        );
        assert_eq!(std::fs::read(path.join("unknown"))?, b"preserve");
        for name in ["settings", "signing-trust", "rollback", "user-document"] {
            assert_eq!(std::fs::read(root.path().join(name))?, b"authored");
        }
        Ok(())
    })
}

#[test]
fn deletion_journal_resumes_after_partial_payload_removal_without_touching_other_entries()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"interrupted deletion");
        let mut lease = open(root.path(), 32, context)?.acquire(
            Namespace::UpdateOffers,
            &key,
            16,
            100,
            context,
        )?;
        lease.write("first", b"one", context)?;
        lease.write("second", b"two", context)?;
        let cache = open(root.path(), 32, context)?;
        let parent = cache.namespace(Namespace::UpdateOffers)?;
        let mut record = lease.record.clone();
        record.deleting = true;
        let journal = parent.path.join(format!("{}.retiring", record.key));
        protection::with(context, |protection| {
            let mut file = protection.file(&journal, context)?;
            file.write_all(&serde_json::to_vec(&record)?)?;
            file.sync_all()
        })?;
        tree::remove_file(&lease.path().join("first"), context)?;
        let path = lease.path();
        drop(lease);
        let report = cache.maintain(100, context)?;
        assert_eq!(report.removed_entries, 1);
        assert!(!path.exists());
        assert!(!journal.exists());
        assert_eq!(cache.maintain(100, context)?.removed_entries, 0);
        Ok(())
    })
}

#[test]
fn settled_opaque_output_is_recorded_and_oversized_output_remains_safely_reclaimable() -> TestResult
{
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"native producer");
        let mut lease = open(root.path(), 16, context)?.acquire(
            Namespace::SetupBrowser,
            &key,
            16,
            100,
            context,
        )?;
        lease.begin_generated("test-native-producer-v1", context)?;
        std::fs::create_dir(lease.path().join("generated"))?;
        std::fs::write(lease.path().join("generated/bytes"), [7; 32])?;
        assert!(lease.record_generated(100, context).is_err());
        assert_eq!(lease.record.files, ["generated/bytes"]);
        let path = lease.path();
        drop(lease);
        let report = open(root.path(), 16, context)?.maintain(100, context)?;
        assert_eq!(report.removed_entries, 1);
        assert!(!path.exists());
        Ok(())
    })
}

#[test]
fn generated_empty_directories_are_owned_and_reclaimed_after_retirement() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut lease = open(root.path(), 16, context)?.acquire(
            Namespace::SetupBrowser,
            &storage_key(b"empty native directories"),
            16,
            100,
            context,
        )?;
        lease.begin_generated("test-empty-native-producer-v1", context)?;
        std::fs::create_dir_all(lease.path().join("empty/nested"))?;
        assert_eq!(lease.record_generated(100, context)?, 0);
        let path = lease.path();
        drop(lease);
        let report = open(root.path(), 16, context)?
            .maintain(100 + Namespace::SetupBrowser.retention(), context)?;
        assert_eq!(report.removed_entries, 1);
        assert!(!path.exists());
        Ok(())
    })
}

#[test]
fn deep_package_ancestors_are_pinned_without_changing_supported_relative_paths() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut lease = open(root.path(), 16, context)?.acquire(
            Namespace::PackedPayloads,
            &storage_key(b"deep package"),
            16,
            100,
            context,
        )?;
        let relative = format!("{}payload", "a/".repeat(40));
        lease.declare([relative.clone()], context)?;
        let path = lease.path().join(&relative);
        std::fs::create_dir_all(path.parent().ok_or("Parent absent")?)?;
        std::fs::write(&path, b"bytes")?;
        assert_eq!(lease.verify_storage(context)?, 5);
        drop(lease);
        assert_eq!(
            open(root.path(), 16, context)?
                .maintain(100 + Namespace::PackedPayloads.retention(), context)?
                .removed_entries,
            1
        );
        assert!(!path.exists());
        Ok(())
    })
}

#[test]
fn durable_native_producer_pin_survives_host_loss_until_feature_owned_retirement_recovery()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"native producer continuity");
        let mut lease = open(root.path(), 64, context)?.acquire(
            Namespace::SetupBrowser,
            &key,
            16,
            100,
            context,
        )?;
        lease.write("payload", b"before", context)?;
        lease.begin_generated("feature-native-recovery-v1", context)?;
        let path = lease.path();
        drop(lease);
        std::fs::write(path.join("payload"), b"native bytes")?;
        let now = 100 + Namespace::SetupBrowser.retention();
        let report = open(root.path(), 64, context)?.maintain(now, context)?;
        assert_eq!(report.removed_entries, 0);
        assert_eq!(report.pinned_entries, 1);
        assert_eq!(report.unfinished_entries, 1);
        assert!(
            open(root.path(), 64, context)?
                .find(Namespace::SetupBrowser, &key, now, context)?
                .is_none()
        );
        let rejected = open(root.path(), 64, context)?.acquire(
            Namespace::SetupBrowser,
            &key,
            16,
            now,
            context,
        );
        assert_eq!(
            rejected.err().ok_or("Unfinished producer was reused")?.kind(),
            io::ErrorKind::WouldBlock
        );
        let mut recovered = open(root.path(), 64, context)?
            .recover_pending(Namespace::SetupBrowser, context)?
            .ok_or("Pending producer absent")?;
        assert_eq!(recovered.producer(), Some("feature-native-recovery-v1"));
        assert!(recovered.write("payload", b"forbidden", context).is_err());
        assert!(recovered.declare(["forbidden".into()], context).is_err());
        assert!(recovered.begin_generated("replacement", context).is_err());
        assert_eq!(recovered.record_generated(100, context)?, 12);
        assert!(recovered.producer().is_none());
        drop(recovered);
        assert_eq!(open(root.path(), 64, context)?.maintain(now, context)?.removed_entries, 1);
        assert!(!path.exists());
        Ok(())
    })
}
#[test]
fn native_output_adoption_requires_a_bounded_prior_grant() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut lease = open(root.path(), 64, context)?.acquire(
            Namespace::SetupBrowser,
            &storage_key(b"native grant bounds"),
            16,
            100,
            context,
        )?;
        assert!(lease.record_generated(100, context).is_err());
        for descriptor in ["", "nul\0descriptor", &"x".repeat(8193)] {
            assert!(lease.begin_generated(descriptor, context).is_err());
        }
        assert!(lease.producer().is_none());
        lease.begin_generated("native-v1", context)?;
        assert_eq!(lease.record_generated(100, context)?, 0);
        assert!(lease.producer().is_none());
        Ok(())
    })
}
