// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Historical reads retain exact pages/stages and reject live, incomplete or foreign content.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use sugarsubstitute_contracts::installation::InstallationId;

fn history(context: &JobContext) -> io::Result<(tempfile::TempDir, Directory)> {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("history");
    protection::with(context, |owner| {
        owner.directory(&path, Mode::CreateNew, context)?;
        append_at(&path, &["operation=historical\n".into()], 0, owner, context)
    })?;
    Ok((temporary, Directory::open(&path, false)?))
}

#[test]
fn passive_history_pins_pages_preserves_incomplete_stages_and_rejects_live_writers() -> TestResult {
    worker::run(|context, _| {
        let (_temporary, root) = history(context)?;
        let page = std::fs::read(root.path().join("page-0.json"))?;
        let stage = "record-stream-0123456789abcdef0123456789abcdef";
        let incomplete = b"{original incomplete stage";
        protection::with(context, |owner| {
            use std::io::Write;
            let mut file = owner.file(&root.path().join(stage), context)?;
            file.write_all(incomplete)?;
            files::sync(&file, context)?;
            owner.close_file(file)
        })?;
        with_closed_source(&root, HistorySource::Original, context, |source, excluded| {
            assert_eq!(excluded, &["writer"]);
            assert_eq!(std::fs::read(source.path().join(stage))?, incomplete);
            assert!(std::fs::write(source.path().join("page-0.json"), b"changed").is_err());
            Ok(())
        })?;
        assert_eq!(std::fs::read(root.path().join(stage))?, incomplete);
        assert_eq!(std::fs::read(root.path().join("page-0.json"))?, page);
        protection::with(context, |owner| {
            let _writer = contention::acquire(owner, &root.path().join("writer"), context)?;
            assert!(
                with_closed_source(&root, HistorySource::Original, context, |_, _| Ok(())).is_err()
            );
            Ok(())
        })?;
        with_closed_source(&root, HistorySource::Original, context, |_, _| Ok(()))?;
        Ok(())
    })
}

#[test]
fn unknown_or_future_historical_bytes_and_missing_controls_are_never_repaired() -> TestResult {
    worker::run(|context, _| {
        let (_temporary, root) = history(context)?;
        let original = std::fs::read(root.path().join("page-0.json"))?;
        let future = b"{\"schema\":2,\"sequence\":1,\"omitted\":0,\"events\":[]}";
        write(&root, "page-0.json", future, context)?;
        assert!(
            with_closed_source(&root, HistorySource::Original, context, |_, _| Ok(())).is_err()
        );
        assert_eq!(std::fs::read(root.path().join("page-0.json"))?, future);
        write(&root, "page-0.json", &original, context)?;
        let unknown = root.path().join("unknown-authored");
        protection::with(context, |owner| {
            use std::io::Write;
            let mut file = owner.file(&unknown, context)?;
            file.write_all(b"authored bytes")?;
            files::sync(&file, context)?;
            owner.close_file(file)
        })?;
        assert!(
            with_closed_source(&root, HistorySource::Original, context, |_, _| Ok(())).is_err()
        );
        assert_eq!(std::fs::read(&unknown)?, b"authored bytes");
        std::fs::remove_file(&unknown)?;
        with_closed_source(&root, HistorySource::Original, context, |_, _| Ok(()))?;
        std::fs::remove_file(root.path().join("writer"))?;
        assert!(
            with_closed_source(&root, HistorySource::Original, context, |_, _| Ok(())).is_err()
        );
        assert!(!root.path().join("writer").exists());
        assert_eq!(std::fs::read(root.path().join("page-0.json"))?, original);
        Ok(())
    })
}

/// Seed a complete earlier operational history at `<parent>/operational-v1`.
fn seed(parent: &Path, context: &JobContext) -> io::Result<Directory> {
    let path = parent.join(DIRECTORY);
    protection::with(context, |owner| {
        owner.directory(&path, Mode::CreateNew, context)?;
        append_at(&path, &["operation=earlier\n".into()], 0, owner, context)
    })?;
    Directory::open(&path, false)
}

/// Every file below `root` with its exact bytes, for before/after comparisons.
fn bytes_below(root: &Path) -> io::Result<std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>> {
    let mut found = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                found.insert(
                    entry.path().strip_prefix(root).map_err(io::Error::other)?.to_owned(),
                    std::fs::read(entry.path())?,
                );
            }
        }
    }
    Ok(found)
}

#[test]
fn earlier_history_is_copied_once_into_the_installation_and_the_original_never_changes()
-> TestResult {
    worker::run(|context, _| {
        let installation = worker::installation(0xe1, context)?;
        let root = Directory::open(installation.path(), false)?;
        let identity = InstallationId::new(format!("{:032x}", 0xe1))?;
        let earlier = tempfile::tempdir()?;
        seed(earlier.path(), context)?;
        let original = bytes_below(earlier.path())?;
        let destination = root.path().join("user/runtime/account-history").join(DIRECTORY);

        import_original(
            &root,
            ImportAssociation::Installed(&identity),
            earlier.path(),
            "account-history",
            context,
        )?;

        let copied = bytes_below(&destination)?;
        assert_eq!(
            copied.get(Path::new("page-0.json")),
            original.get(&Path::new(DIRECTORY).join("page-0.json"))
        );
        assert!(
            !copied.contains_key(Path::new("writer")),
            "the original writer control is never copied"
        );
        assert_eq!(
            bytes_below(earlier.path())?,
            original,
            "the original history stays byte-identical"
        );

        // A later edit to the imported pages belongs to the installation and survives a re-run.
        std::fs::write(destination.join("page-0.json"), b"edited after import")?;
        import_original(
            &root,
            ImportAssociation::Installed(&identity),
            earlier.path(),
            "account-history",
            context,
        )?;
        assert_eq!(std::fs::read(destination.join("page-0.json"))?, b"edited after import");
        assert_eq!(bytes_below(earlier.path())?, original);

        // A completed import is verified on its own record, so an older application that wakes up
        // and holds the original writer again cannot block or redo it.
        protection::with(context, |owner| {
            let _writer = contention::acquire(
                owner,
                &earlier.path().join(DIRECTORY).join("writer"),
                context,
            )?;
            import_original(
                &root,
                ImportAssociation::Installed(&identity),
                earlier.path(),
                "account-history",
                context,
            )
        })?;
        assert_eq!(std::fs::read(destination.join("page-0.json"))?, b"edited after import");
        Ok(())
    })
}

#[test]
fn absent_earlier_history_imports_nothing_and_each_provenance_stays_separate() -> TestResult {
    worker::run(|context, _| {
        let installation = worker::installation(0xe2, context)?;
        let root = Directory::open(installation.path(), false)?;
        let identity = InstallationId::new(format!("{:032x}", 0xe2))?;
        let none = tempfile::tempdir()?;
        import_original(
            &root,
            ImportAssociation::Installed(&identity),
            none.path(),
            "account-history",
            context,
        )?;
        assert!(!root.path().join("user/runtime/account-history").join(DIRECTORY).exists());

        let account = tempfile::tempdir()?;
        let earlier_installation = tempfile::tempdir()?;
        seed(account.path(), context)?;
        seed(earlier_installation.path(), context)?;
        import_original(
            &root,
            ImportAssociation::Installed(&identity),
            account.path(),
            "account-history",
            context,
        )?;
        import_original(
            &root,
            ImportAssociation::Installed(&identity),
            earlier_installation.path(),
            "installation-history",
            context,
        )?;
        for provenance in ["account-history", "installation-history"] {
            assert!(
                root.path()
                    .join("user/runtime")
                    .join(provenance)
                    .join(DIRECTORY)
                    .join("page-0.json")
                    .is_file()
            );
        }
        Ok(())
    })
}

#[test]
fn a_live_original_writer_blocks_the_import_until_it_is_released() -> TestResult {
    worker::run(|context, _| {
        let installation = worker::installation(0xe3, context)?;
        let root = Directory::open(installation.path(), false)?;
        let identity = InstallationId::new(format!("{:032x}", 0xe3))?;
        let earlier = tempfile::tempdir()?;
        let history = seed(earlier.path(), context)?;
        let original = bytes_below(earlier.path())?;
        let destination = root.path().join("user/runtime/account-history").join(DIRECTORY);
        protection::with(context, |owner| {
            let _writer = contention::acquire(owner, &history.path().join("writer"), context)?;
            assert!(
                import_original(
                    &root,
                    ImportAssociation::Installed(&identity),
                    earlier.path(),
                    "account-history",
                    context
                )
                .is_err()
            );
            Ok(())
        })?;
        assert!(!destination.exists(), "a refused import publishes nothing");
        assert_eq!(bytes_below(earlier.path())?, original);
        import_original(
            &root,
            ImportAssociation::Installed(&identity),
            earlier.path(),
            "account-history",
            context,
        )?;
        assert!(destination.join("page-0.json").is_file());
        Ok(())
    })
}
