// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual selected-folder custody through the existing execution owner.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};

#[test]
fn selected_records_support_long_local_paths_through_native_publication() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = temporary.path().join("parent".repeat(25)).join("middle".repeat(25));
        std::fs::create_dir_all(&parent)?;
        let selected = parent.join("ComfyUI");
        assert!(selected.as_os_str().len() > 260);
        let directory = RecordDirectory::open(&selected, true, true, context)?;
        directory.create(None, "anchor", b"immutable", context)?;
        directory.create(None, "record", b"original", context)?;
        let original = directory.read(None, "record", context)?;
        directory.replace("anchor", None, "record", &original, b"replacement", context)?;
        assert_eq!(directory.read(None, "record", context)?.as_slice(), b"replacement");
        Ok(())
    })
}

#[test]
fn read_only_presence_does_not_establish_directory_ownership_or_create_state() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let missing = temporary.path().join("missing");
        assert!(RecordDirectory::record_absent(&missing, "record", context)?);
        assert!(!missing.exists());
        assert!(RecordDirectory::record_absent(temporary.path(), "record", context)?);
        std::fs::write(temporary.path().join("record"), b"unknown")?;
        assert!(!RecordDirectory::record_absent(temporary.path(), "record", context)?);
        assert_eq!(std::fs::read(temporary.path().join("record"))?, b"unknown");
        assert!(RecordDirectory::record_absent(temporary.path(), "../record", context).is_err());
        Ok(())
    })
}

#[test]
fn read_only_custody_never_creates_or_publishes() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let missing = temporary.path().join("missing");
        assert!(RecordDirectory::open(&missing, true, false, context).is_err());
        assert!(!missing.exists());
        let directory = RecordDirectory::open(temporary.path(), false, false, context)?;
        assert!(directory.create(None, "record", b"x", context).is_err());
        assert!(directory.create_child("child", context).is_err());
        assert!(std::fs::read_dir(temporary.path())?.next().is_none());
        Ok(())
    })
}
#[test]
fn only_the_selected_leaf_can_be_created_and_ancestors_stay_pinned() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        assert!(
            RecordDirectory::open(&temporary.path().join("missing/child"), true, true, context)
                .is_err()
        );
        assert!(!temporary.path().join("missing").exists());
        let selected = temporary.path().join("糖😀");
        let directory = RecordDirectory::open(&selected, true, true, context)?;
        assert!(std::fs::rename(&selected, temporary.path().join("moved")).is_err());
        let child = directory.create_child("attempt", context)?;
        directory.create(Some("attempt"), "record", b"complete", context)?;
        assert_eq!(directory.read(Some("attempt"), "record", context)?.as_slice(), b"complete");
        assert!(directory.read(None, "attempt/record", context).is_err());
        assert!(directory.create_child("../escape", context).is_err());
        assert!(directory.create_child("attempt", context).is_err());
        assert_eq!(std::fs::read(child.join("record"))?, b"complete");
        Ok(())
    })
}
#[test]
fn create_only_and_exact_original_replacement_preserve_unknown_bytes() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = RecordDirectory::open(temporary.path(), false, true, context)?;
        directory.create(None, "anchor", b"immutable", context)?;
        directory.create(None, "record", b"first", context)?;
        let first = directory.read(None, "record", context)?;
        assert!(directory.create(None, "record", b"other", context).is_err());
        std::fs::write(temporary.path().join("record"), b"changed")?;
        assert!(directory.replace("anchor", None, "record", &first, b"other", context).is_err());
        assert_eq!(std::fs::read(temporary.path().join("record"))?, b"changed");
        let changed = directory.read(None, "record", context)?;
        directory.replace("anchor", None, "record", &changed, b"second", context)?;
        assert_eq!(directory.read(None, "record", context)?.as_slice(), b"second");
        let abandoned = temporary.path().join(".record-stage-interrupted");
        std::fs::write(&abandoned, b"partial")?;
        let second = directory.read(None, "record", context)?;
        directory.replace("anchor", None, "record", &second, b"third", context)?;
        assert_eq!(std::fs::read(abandoned)?, b"partial");
        Ok(())
    })
}
#[test]
fn full_record_bound_and_overflow_preserve_original_content() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = RecordDirectory::open(temporary.path(), false, true, context)?;
        directory.create(None, "anchor", b"immutable", context)?;
        let exact = vec![b'x'; WORKER_RECORD_BYTES];
        directory.create(None, "record", &exact, context)?;
        assert_eq!(directory.read(None, "record", context)?.as_slice(), &exact);
        let snapshot = directory.read(None, "record", context)?;
        let over = vec![b'x'; WORKER_RECORD_BYTES + 1];
        assert!(directory.replace("anchor", None, "record", &snapshot, &over, context).is_err());
        assert_eq!(std::fs::read(temporary.path().join("record"))?, exact);
        std::fs::write(temporary.path().join("oversize"), over)?;
        assert!(directory.read(None, "oversize", context).is_err());
        Ok(())
    })
}
#[test]
fn busy_anchor_and_hard_link_records_are_preserved() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = RecordDirectory::open(temporary.path(), false, true, context)?;
        directory.create(None, "anchor", b"immutable", context)?;
        directory.create(None, "record", b"original", context)?;
        let snapshot = directory.read(None, "record", context)?;
        let writer = directory.root.read("anchor")?;
        writer.try_lock().map_err(io::Error::from)?;
        assert!(directory.replace("anchor", None, "record", &snapshot, b"other", context).is_err());
        assert_eq!(std::fs::read(temporary.path().join("record"))?, b"original");
        drop(writer);
        std::fs::write(temporary.path().join("authored"), b"preserve")?;
        std::fs::hard_link(temporary.path().join("authored"), temporary.path().join("alias"))?;
        assert!(directory.read(None, "alias", context).is_err());
        assert!(directory.replace("anchor", None, "alias", &snapshot, b"other", context).is_err());
        assert_eq!(std::fs::read(temporary.path().join("authored"))?, b"preserve");
        Ok(())
    })
}
#[test]
fn inventories_are_paged_and_reject_overflow_without_removing_entries() -> TestResult {
    let temporary = tempfile::tempdir()?;
    for index in 0..4097 {
        std::fs::write(temporary.path().join(format!("item-{index}")), b"")?;
    }
    let selected = temporary.path().to_owned();
    worker::run_reserved(4 * 1024 * 1024, move |context, _| {
        let mut directory = RecordDirectory::open(&selected, false, false, context)?;
        assert!(directory.page(false, context).is_err());
        assert_eq!(directory.page(true, context)?.len(), RECORD_PAGE_ENTRIES);
        for _ in 1..128 {
            assert_eq!(directory.page(false, context)?.len(), RECORD_PAGE_ENTRIES);
        }
        assert!(directory.page(false, context).is_err());
        assert_eq!(std::fs::read_dir(&selected)?.count(), 4097);
        Ok(())
    })
}
#[test]
fn insufficient_reservation_rejects_custody_before_directory_creation() -> TestResult {
    worker::run_reserved(1, |context, _| {
        let temporary = tempfile::tempdir()?;
        let selected = temporary.path().join("selected");
        assert!(RecordDirectory::open(&selected, true, true, context).is_err());
        assert!(!selected.exists());
        Ok(())
    })
}
#[test]
fn expiry_rejects_new_mutations_and_preserves_completed_records() -> TestResult {
    worker::run_reserved_expiring(4 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let directory = RecordDirectory::open(temporary.path(), false, true, context)?;
        directory.create(None, "anchor", b"immutable", context)?;
        directory.create(None, "record", b"original", context)?;
        let snapshot = directory.read(None, "record", context)?;
        clock.expire();
        assert!(directory.replace("anchor", None, "record", &snapshot, b"other", context).is_err());
        assert_eq!(std::fs::read(temporary.path().join("record"))?, b"original");
        Ok(())
    })
}

#[test]
fn identical_bytes_at_another_native_identity_cannot_replace_the_original_record() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = RecordDirectory::open(temporary.path(), false, true, context)?;
        directory.create(None, "anchor", b"immutable", context)?;
        directory.create(None, "record", b"original", context)?;
        let snapshot = directory.read(None, "record", context)?;
        let other = directory.create_child("other", context)?;
        directory.create(Some("other"), "record", b"original", context)?;
        assert!(
            directory
                .replace("anchor", Some("other"), "record", &snapshot, b"changed", context)
                .is_err()
        );
        assert_eq!(std::fs::read(other.join("record"))?, b"original");
        let target = temporary.path().join("record");
        let preserved = temporary.path().join("preserved-original");
        std::fs::hard_link(&target, &preserved)?;
        std::fs::remove_file(&target)?;
        directory.create(None, "record", b"original", context)?;
        let substituted = directory.read(None, "record", context)?;
        assert_ne!(snapshot.identity, substituted.identity);
        assert!(
            directory.replace("anchor", None, "record", &snapshot, b"changed", context).is_err()
        );
        assert_eq!(std::fs::read(target)?, b"original");
        assert_eq!(std::fs::read(preserved)?, b"original");
        Ok(())
    })
}

/// A `TEMP` such as `C:\Users\RUNNER~1\AppData\Local\Temp` spells record parents with 8.3 short
/// names. Those are the same folders, so custody opens there and keeps the resolved long spelling.
#[test]
fn short_name_parents_open_at_their_resolved_long_location() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let long = temporary.path().join("Record parent with a long name");
        std::fs::create_dir(&long)?;
        let short = files::alias_fixture::short_spelling(&long)?;
        assert_ne!(short.file_name(), long.file_name(), "volume generates no 8.3 short names");
        let expected = Directory::open(&long, false)?.resolved()?.path().join("ComfyUI");
        let verbatim = PathBuf::from(format!(r"\\?\{}", short.display()));
        assert!(RecordDirectory::record_absent(&short.join("ComfyUI"), "record", context)?);
        let directory = RecordDirectory::open(&short.join("ComfyUI"), true, true, context)?;
        assert_eq!(directory.path(), expected);
        directory.create(None, "record", b"through the short name", context)?;
        drop(directory);
        assert!(!RecordDirectory::record_absent(&verbatim.join("ComfyUI"), "record", context)?);
        let directory = RecordDirectory::open(&verbatim.join("ComfyUI"), false, false, context)?;
        assert_eq!(directory.path(), expected);
        assert_eq!(directory.read(None, "record", context)?.as_slice(), b"through the short name");
        assert_eq!(std::fs::read(long.join("ComfyUI").join("record"))?, b"through the short name");
        Ok(())
    })
}

/// A drive alias, as `subst` defines, reaches the same folder from another drive. Record custody
/// still refuses a parent below it. (A parent that is a drive root is refused by lexical intake.)
#[test]
fn drive_alias_parents_stay_refused_without_creating_records() -> TestResult {
    worker::run_reserved(4 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        std::fs::create_dir(temporary.path().join("aliased"))?;
        let target = Directory::open(temporary.path(), false)?.resolved()?;
        let alias = files::alias_fixture::DriveAlias::define(target.path())?;
        let refused =
            refuse_redirected_parent(&alias.root().join("aliased").join("ComfyUI"), context);
        alias.remove()?;
        refused?;
        assert!(!temporary.path().join("aliased").join("ComfyUI").exists());
        Ok(())
    })
}

/// The alias parent itself opens and pins, so only the location comparison can refuse custody.
fn refuse_redirected_parent(selected: &Path, context: &JobContext) -> TestResult {
    Directory::open(selected.parent().ok_or("Selected parent absent")?, false)?;
    for outcome in [
        RecordDirectory::open(selected, true, true, context).map(drop),
        RecordDirectory::record_absent(selected, "record", context).map(drop),
    ] {
        let refusal = outcome.err().ok_or("Redirected record parent was accepted")?;
        assert_eq!(refusal.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(refusal.to_string(), "Record parent must use its resolved local location");
    }
    Ok(())
}
