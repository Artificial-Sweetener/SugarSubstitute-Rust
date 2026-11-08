// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Earlier account and installation layouts migrate into `user` and no original ever changes.
use super::*;
use crate::{
    durable_storage::imports::ImportAssociation,
    setup_storage::test_worker::{self, TestResult},
};
use std::{collections::BTreeMap, path::PathBuf};
use sugarsubstitute_contracts::lifecycle::NamespaceId;

/// Every file below `root` with its exact bytes, for before/after comparisons.
fn bytes_below(root: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                let relative =
                    entry.path().strip_prefix(root).map_err(io::Error::other)?.to_owned();
                found.insert(relative, std::fs::read(entry.path())?);
            }
        }
    }
    Ok(found)
}

#[test]
fn earlier_account_and_installation_history_migrate_into_user_and_nothing_else_changes()
-> TestResult {
    // The production preparation job's own reservation, less the retained custody it claims first.
    test_worker::run_reserved(LAUNCH_DATA_PREPARATION_BYTES, |context, _| {
        let _retained = context.memory().claim(RETAINED_BYTES)?;
        let fixture = Fixture::new(context)?;
        let installation = fixture.installation(context)?;
        // The account's earlier `SugarSubstitute\runtime` and an unrelated neighbor stand in for
        // the profile folders; neither may change and nothing may be created beside them.
        let profile = tempfile::tempdir()?;
        let account_runtime = profile.path().join("AppData/Local/SugarSubstitute/runtime");
        std::fs::create_dir_all(&account_runtime)?;
        crate::diagnostic_storage::seed_earlier_history(&account_runtime, context)?;
        // The earlier installation created its own runtime folder through the protected owner.
        protection::with(context, |owner| {
            owner.directory(&installation.root.path().join("runtime"), Mode::CreateNew, context)?;
            Ok(())
        })?;
        crate::diagnostic_storage::seed_earlier_history(
            &installation.root.path().join("runtime"),
            context,
        )?;
        std::fs::write(profile.path().join("neighbor.txt"), b"unrelated profile bytes")?;
        let profile_before = bytes_below(profile.path())?;
        let root_before = bytes_below(installation.root.path())?;

        let association = ImportAssociation::Installed(installation.snapshot.installation());
        runtime::settle_sources(
            &installation.root,
            association,
            vec![NamespaceId::new([7; 32])],
            &account_runtime,
            context,
        )?;

        assert_eq!(bytes_below(profile.path())?, profile_before, "the profile is never written");
        let root_after = bytes_below(installation.root.path())?;
        for (path, bytes) in &root_before {
            assert_eq!(root_after.get(path), Some(bytes), "{path:?} changed");
        }
        for path in root_after.keys().filter(|path| !root_before.contains_key(*path)) {
            assert!(path.starts_with("user"), "{path:?} was created outside the user folder");
        }
        for provenance in ["account-history", "installation-history"] {
            let imported = installation
                .root
                .path()
                .join("user/runtime")
                .join(provenance)
                .join("operational-v1/page-0.json");
            assert_eq!(
                std::fs::read(&imported)?,
                std::fs::read(account_runtime.join("operational-v1/page-0.json"))?,
                "{provenance}"
            );
        }

        // A repeated settlement is a verified no-op, even after the profile stops existing.
        let settled = bytes_below(installation.root.path())?;
        drop(profile);
        runtime::settle_sources(
            &installation.root,
            association,
            vec![NamespaceId::new([7; 32])],
            Path::new(r"Z:\no\such\profile"),
            context,
        )?;
        assert_eq!(bytes_below(installation.root.path())?, settled);
        Ok(())
    })
}

#[test]
fn earlier_input_archives_migrate_with_exact_bytes_and_are_classified_once() -> TestResult {
    test_worker::run(|context, _| {
        let fixture = Fixture::new(context)?;
        let installation = fixture.installation(context)?;
        crate::durable_storage::user_paths::create_parents(
            &installation.root,
            &installation.root.path().join("user"),
            context,
        )?;
        let source = installation.root.path().join("desktop-data/input-canvas");
        std::fs::create_dir_all(&source)?;
        let rejected = format!("input-documents.rejected-{}.ssdoc", "ab".repeat(32));
        let authored = [
            ("input-documents.ssdoc", b"current archive bytes".as_slice()),
            ("input-documents.previous.ssdoc", b"previous archive bytes".as_slice()),
            (rejected.as_str(), b"rejected archive bytes".as_slice()),
            ("notes.txt", b"authored note".as_slice()),
        ];
        for (name, bytes) in authored {
            std::fs::write(source.join(name), bytes)?;
        }
        let before = bytes_below(&source)?;
        let association = ImportAssociation::Installed(installation.snapshot.installation());
        let inspected = std::cell::RefCell::new(Vec::new());
        let mut classify =
            |path: &Path, _: &JobContext| -> io::Result<HistoricalArchiveDisposition> {
                inspected.borrow_mut().push(path.to_owned());
                Ok(HistoricalArchiveDisposition::PreservedOriginal)
            };

        host::import_archives(&installation.root, association, &mut classify, context)?;

        let target = installation.root.path().join("user/input-canvas");
        assert_eq!(bytes_below(&target)?, before, "every entry is copied byte for byte");
        assert_eq!(bytes_below(&source)?, before, "the earlier folder never changes");
        assert_eq!(
            inspected.borrow().len(),
            3,
            "only recognized archive names reach the archive owner"
        );

        std::fs::write(target.join("input-documents.ssdoc"), b"edited in the new location")?;
        host::import_archives(&installation.root, association, &mut classify, context)?;
        assert_eq!(inspected.borrow().len(), 3, "a completed import is not classified again");
        assert_eq!(
            std::fs::read(target.join("input-documents.ssdoc"))?,
            b"edited in the new location"
        );
        assert_eq!(bytes_below(&source)?, before);
        Ok(())
    })
}
