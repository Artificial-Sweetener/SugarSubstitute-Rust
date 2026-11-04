// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual protected rollover, restart, export and preservation on admitted native workers.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};

#[test]
fn native_rollover_restart_and_export_keep_only_eight_complete_pages() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("operational");
        protection::with(context, |protection| {
            protection.directory(&path, Mode::CreateNew, context)?;
            for sequence in 1..=12 {
                append_at(
                    &path,
                    &[format!("operation=fixture sequence={sequence}\n")],
                    sequence,
                    protection,
                    context,
                )?;
            }
            let export = export_at(&path, protection, context)?;
            let text = std::str::from_utf8(export.as_bytes()).map_err(io::Error::other)?;
            assert!(!text.contains("sequence=4\n"));
            assert!(text.contains("sequence=5\n"));
            assert!(text.contains("sequence=12\n"));
            assert!(text.find("sequence=5\n") < text.find("sequence=12\n"));
            assert_eq!(text.matches("operation=fixture").count(), PAGES);
            assert_eq!(std::fs::read_dir(&path)?.count(), PAGES + 2);
            Ok(())
        })?;
        // New native account/directory custody simulates an independent process opening persisted history.
        protection::with(context, |protection| {
            append_at(&path, &["operation=restart\n".into()], 0, protection, context)?;
            let export = export_at(&path, protection, context)?;
            assert!(export.as_bytes().ends_with(b"operation=restart\n"));
            Ok(())
        })?;
        Ok(())
    })
}
#[test]
fn unknown_siblings_and_incident_records_are_preserved_before_rotation() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("operational");
        protection::with(context, |protection| {
            protection.directory(&path, Mode::CreateNew, context)?;
            append_at(&path, &["operation=before\n".into()], 0, protection, context)?;
            let page = std::fs::read(path.join("page-0.json"))?;
            std::fs::write(path.join("incident-summary"), b"authoritative")?;
            assert!(
                append_at(&path, &["operation=after\n".into()], 0, protection, context).is_err()
            );
            assert_eq!(std::fs::read(path.join("incident-summary"))?, b"authoritative");
            assert_eq!(std::fs::read(path.join("page-0.json"))?, page);
            Ok(())
        })?;
        Ok(())
    })
}
#[test]
fn unknown_schema_original_bytes_survive_rejected_append_and_export() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("operational");
        protection::with(context, |protection| {
            protection.directory(&path, Mode::CreateNew, context)?;
            append_at(&path, &["operation=before\n".into()], 0, protection, context)?;
            let root = Directory::open(&path, false)?;
            let future = b"{\"schema\":2,\"sequence\":1,\"omitted\":0,\"events\":[]}";
            write(&root, "page-0.json", future, context)?;
            assert!(
                append_at(&path, &["operation=after\n".into()], 0, protection, context).is_err()
            );
            assert!(export_at(&path, protection, context).is_err());
            assert_eq!(read(&root, "page-0.json", context)?, future);
            Ok(())
        })?;
        Ok(())
    })
}
#[test]
fn native_writer_contention_and_overload_are_observable_without_new_pages() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("operational");
        protection::with(context, |protection| {
            protection.directory(&path, Mode::CreateNew, context)?;
            assert!(
                append_at(
                    &path,
                    &["x".repeat(OPERATIONAL_EVENT_BYTES + 1)],
                    0,
                    protection,
                    context
                )
                .is_err()
            );
            assert_eq!(std::fs::read_dir(&path)?.count(), 0);
            for events in [
                vec!["invalid\u{0001}".into()],
                vec!["bounded".into(); DIAGNOSTIC_BATCH_EVENTS + 1],
            ] {
                assert!(append_at(&path, &events, 0, protection, context).is_err());
                assert_eq!(std::fs::read_dir(&path)?.count(), 0);
            }
            let pin = protection.exclusive_rendezvous(&path.join("writer"), context)?;
            let busy = append_at(&path, &["bounded".into()], 0, protection, context)
                .err()
                .ok_or_else(|| io::Error::other("Expected native writer contention"))?;
            assert!(writer_busy(&busy));
            assert_eq!(std::fs::read_dir(&path)?.count(), 1);
            drop(pin);
            append_at(&path, &["bounded".into()], 0, protection, context)?;
            let expanded = vec!["\"".repeat(OPERATIONAL_EVENT_BYTES); DIAGNOSTIC_BATCH_EVENTS];
            append_at(&path, &expanded, 0, protection, context)?;
            let exported = export_at(&path, protection, context)?;
            assert!(exported.as_bytes().ends_with(format!("{}\n", expanded[0]).as_bytes()));
            Ok(())
        })?;
        Ok(())
    })
}
/// A writer terminated or cancelled between stage creation and publication leaves its exact
/// stage behind. The next writer retires it under writer custody and keeps every page.
#[test]
fn interrupted_page_stage_is_retired_under_writer_custody_and_pages_survive() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("operational");
        protection::with(context, |protection| {
            protection.directory(&path, Mode::CreateNew, context)?;
            append_at(&path, &["operation=before\n".into()], 0, protection, context)?;
            let page = std::fs::read(path.join("page-0.json"))?;
            let root = Directory::open(&path, false)?;
            let stage = StreamStage::plan(&root, "page-1.json", 64, context)?;
            let name = stage.stage_name()?.to_owned();
            let mut interrupted = stage.create(context)?;
            interrupted.append(b"{\"schema\":1,", context)?;
            drop(interrupted);
            assert!(path.join(&name).exists());
            append_at(&path, &["operation=after\n".into()], 0, protection, context)?;
            assert!(!path.join(&name).exists());
            assert_eq!(std::fs::read(path.join("page-0.json"))?, page);
            let export = export_at(&path, protection, context)?;
            let text = std::str::from_utf8(export.as_bytes()).map_err(io::Error::other)?;
            assert!(text.contains("operation=before\n"));
            assert!(text.ends_with("operation=after\n"));
            assert_eq!(std::fs::read_dir(&path)?.count(), 4);
            // Repeated interruptions past one bounded inventory page are all observed and retired.
            for _ in 0..40 {
                drop(StreamStage::plan(&root, "page-2.json", 8, context)?.create(context)?);
            }
            append_at(&path, &["operation=later\n".into()], 0, protection, context)?;
            assert_eq!(std::fs::read_dir(&path)?.count(), 5);
            // An interrupted stage left before any page or owner exists is also retired.
            let fresh = temporary.path().join("fresh");
            protection.directory(&fresh, Mode::CreateNew, context)?;
            let root = Directory::open(&fresh, false)?;
            drop(StreamStage::plan(&root, "owner", 8, context)?.create(context)?);
            append_at(&fresh, &["operation=first\n".into()], 0, protection, context)?;
            assert_eq!(std::fs::read(fresh.join("owner"))?, MARKER);
            Ok(())
        })?;
        Ok(())
    })
}
#[test]
fn stage_lookalikes_and_oversized_stages_are_preserved_and_fail_closed() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("operational");
        protection::with(context, |protection| {
            protection.directory(&path, Mode::CreateNew, context)?;
            append_at(&path, &["operation=before\n".into()], 0, protection, context)?;
            let identity = "0123456789abcdef0123456789abcdef";
            for (name, directory) in [
                ("record-stream-foreign".to_owned(), false),
                (format!("record-stream-{}", identity.to_uppercase()), false),
                (format!("record-stream-{identity}"), true),
            ] {
                let target = path.join(&name);
                if directory {
                    std::fs::create_dir(&target)?;
                } else {
                    std::fs::write(&target, b"preserved")?;
                }
                assert!(
                    append_at(&path, &["operation=after\n".into()], 0, protection, context)
                        .is_err()
                );
                assert!(export_at(&path, protection, context).is_err());
                assert!(target.exists());
                if directory {
                    std::fs::remove_dir(&target)?;
                } else {
                    assert_eq!(std::fs::read(&target)?, b"preserved");
                    std::fs::remove_file(&target)?;
                }
            }
            let root = Directory::open(&path, false)?;
            let stage = StreamStage::plan(&root, "page-1.json", PAGE_BYTES as u64 + 1, context)?;
            let name = stage.stage_name()?.to_owned();
            let mut oversized = stage.create(context)?;
            for chunk in vec![b'x'; PAGE_BYTES + 1].chunks(8192) {
                oversized.append(chunk, context)?;
            }
            drop(oversized);
            assert!(
                append_at(&path, &["operation=after\n".into()], 0, protection, context).is_err()
            );
            assert_eq!(std::fs::metadata(path.join(&name))?.len(), PAGE_BYTES as u64 + 1);
            Ok(())
        })?;
        Ok(())
    })
}
