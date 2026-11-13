// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify ownership admission, exact deletion, preservation and tombstone resumption.
use super::*;
use crate::durable_storage::files::reparse_fixture;
use crate::setup_storage::{
    owned_retirement_root,
    test_worker::{self, TestResult},
};
use std::path::{Path, PathBuf};

/// An owned installation with a feature folder below its user directory.
fn owned(
    context: &JobContext,
) -> Result<(tempfile::TempDir, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    let installation = test_worker::installation(1, context)?;
    let folder = installation.path().join("user").join("feature");
    std::fs::create_dir_all(&folder)?;
    Ok((installation, folder))
}

/// A folder holding `app/one.dll`, `app/nested/two.dll` and a top-level manifest.
fn generation(folder: &Path) -> std::io::Result<PathBuf> {
    let generation = folder.join("generation");
    std::fs::create_dir_all(generation.join("app").join("nested"))?;
    std::fs::write(generation.join("app").join("one.dll"), b"one")?;
    std::fs::write(generation.join("app").join("nested").join("two.dll"), b"two")?;
    std::fs::write(generation.join("manifest.json"), b"app/one.dll\napp/nested/two.dll")?;
    Ok(generation)
}

fn inventory() -> Result<TreeInventory, RetirementError> {
    TreeInventory::new(["app/one.dll".to_owned(), "app/nested/two.dll".to_owned()])?
        .finally(["manifest.json".to_owned()])
}

/// Rebuild the inventory from the tombstone's manifest, as a resuming owner does.
fn from_manifest(
    tombstone: &Tombstone<'_>,
    context: &JobContext,
) -> Result<TreeInventory, RetirementError> {
    let names = tombstone.read("manifest.json", 4096, context)?.unwrap_or_default();
    let names = String::from_utf8(names).map_err(|_| RetirementError::Invalid("manifest"))?;
    TreeInventory::new(names.lines().map(str::to_owned))?.finally(["manifest.json".to_owned()])
}

fn entries(folder: &Path) -> std::io::Result<Vec<String>> {
    let mut names = std::fs::read_dir(folder)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    Ok(names)
}

#[test]
fn ownership_requires_a_verified_installation_and_a_folder_confined_below_it() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        assert!(owned_retirement_root(installation.path(), &folder, context).is_ok());
        assert!(matches!(
            owned_retirement_root(installation.path(), installation.path(), context),
            Err(RetirementError::Ownership(_))
        ));
        let state = installation.path().join("state");
        assert!(matches!(
            owned_retirement_root(installation.path(), &state, context),
            Err(RetirementError::Ownership(_))
        ));
        let elsewhere = tempfile::tempdir()?;
        assert!(matches!(
            owned_retirement_root(installation.path(), elsewhere.path(), context),
            Err(RetirementError::Ownership(_))
        ));
        let unowned = tempfile::tempdir()?;
        std::fs::create_dir(unowned.path().join("feature"))?;
        assert!(matches!(
            owned_retirement_root(unowned.path(), &unowned.path().join("feature"), context),
            Err(RetirementError::Ownership(_))
        ));
        // A junction below the installation never confers ownership of its target.
        let outside = tempfile::tempdir()?;
        let link = installation.path().join("user").join("redirected");
        reparse_fixture::junction(&link, outside.path(), context)?;
        assert!(owned_retirement_root(installation.path(), &link, context).is_err());
        Ok(())
    })
}

#[test]
fn a_file_is_removed_only_when_regular_single_linked_and_not_in_use() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        std::fs::write(folder.join("retired.image"), b"bytes")?;
        let reader = std::fs::File::open(folder.join("retired.image"))?;
        assert_eq!(
            root.retire("retired.image", &RetirementUnit::File, context)?.outcome,
            RetirementOutcome::Busy
        );
        drop(reader);
        assert_eq!(
            root.retire("retired.image", &RetirementUnit::File, context)?.outcome,
            RetirementOutcome::Removed
        );
        assert_eq!(
            root.retire("retired.image", &RetirementUnit::File, context)?.outcome,
            RetirementOutcome::Absent
        );
        std::fs::write(folder.join("linked.image"), b"kept")?;
        std::fs::hard_link(folder.join("linked.image"), folder.join("authored"))?;
        assert_eq!(
            root.retire("linked.image", &RetirementUnit::File, context)?.outcome,
            RetirementOutcome::Retained { preserved: 1, busy: 0 }
        );
        assert_eq!(std::fs::read(folder.join("linked.image"))?, b"kept");
        std::fs::create_dir(folder.join("folder.image"))?;
        assert_eq!(
            root.retire("folder.image", &RetirementUnit::File, context)?.outcome,
            RetirementOutcome::Retained { preserved: 1, busy: 0 }
        );
        assert!(folder.join("folder.image").is_dir());
        for leaf in ["../outside", "nested/name", ".retired-00000000000000000000000000000000.x"] {
            assert!(matches!(
                root.retire(leaf, &RetirementUnit::File, context),
                Err(RetirementError::Invalid(_))
            ));
        }
        Ok(())
    })
}

#[test]
fn a_tree_loses_only_inventoried_files_and_keeps_unknown_content_in_its_tombstone() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        generation(&folder)?;
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Removed
        );
        assert!(entries(&folder)?.is_empty());

        let generation = self::generation(&folder)?;
        std::fs::write(generation.join("app").join("authored.txt"), b"user")?;
        std::fs::create_dir(generation.join("capture"))?;
        std::fs::write(generation.join("capture").join("report"), b"crash")?;
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Retained { preserved: 2, busy: 0 }
        );
        let names = entries(&folder)?;
        assert_eq!(names.len(), 1);
        assert!(super::tombstone::is_tombstone(&names[0]) && names[0].ends_with(".generation"));
        let tombstone = folder.join(&names[0]);
        assert_eq!(std::fs::read(tombstone.join("app").join("authored.txt"))?, b"user");
        assert_eq!(std::fs::read(tombstone.join("capture").join("report"))?, b"crash");
        assert!(!tombstone.join("app").join("one.dll").exists());
        assert!(!tombstone.join("app").join("nested").exists());
        assert!(!tombstone.join("manifest.json").exists());
        Ok(())
    })
}

#[test]
fn a_junction_inside_a_tree_is_kept_and_its_target_is_never_followed() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        let generation = generation(&folder)?;
        let outside = tempfile::tempdir()?;
        std::fs::write(outside.path().join("one.dll"), b"elsewhere")?;
        std::fs::create_dir(outside.path().join("nested"))?;
        std::fs::write(outside.path().join("nested").join("two.dll"), b"elsewhere")?;
        std::fs::remove_dir_all(generation.join("app"))?;
        reparse_fixture::junction(&generation.join("app"), outside.path(), context)?;
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Retained { preserved: 1, busy: 0 }
        );
        assert_eq!(std::fs::read(outside.path().join("one.dll"))?, b"elsewhere");
        assert_eq!(std::fs::read(outside.path().join("nested").join("two.dll"))?, b"elsewhere");
        // A junction in place of the whole folder is kept as well.
        reparse_fixture::junction(&folder.join("linked"), outside.path(), context)?;
        assert_eq!(
            root.retire("linked", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Retained { preserved: 1, busy: 0 }
        );
        assert_eq!(std::fs::read(outside.path().join("one.dll"))?, b"elsewhere");
        Ok(())
    })
}

#[test]
fn a_tree_with_an_open_file_is_left_untouched() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        let generation = generation(&folder)?;
        let running = std::fs::File::open(generation.join("app").join("nested").join("two.dll"))?;
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Busy
        );
        assert_eq!(entries(&folder)?, ["generation"]);
        assert_eq!(std::fs::read(generation.join("app").join("one.dll"))?, b"one");
        drop(running);
        let pinned = Directory::open(&generation, false)?;
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Busy
        );
        drop(pinned);
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context)?.outcome,
            RetirementOutcome::Removed
        );
        Ok(())
    })
}

#[test]
fn a_crash_after_the_tombstone_rename_resumes_from_the_tombstone_records() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        generation(&folder)?;
        // The crash: the rename committed the retirement, then the process ended.
        let name = super::tombstone::name("generation")?;
        std::fs::rename(folder.join("generation"), folder.join(&name))?;
        // Part of the deletion had also happened.
        std::fs::remove_file(folder.join(&name).join("app").join("one.dll"))?;
        std::fs::write(folder.join(".retired-not-a-tombstone"), b"kept")?;
        let mut seen = Vec::new();
        let report = root.resume(
            &mut |tombstone| {
                seen.push(tombstone.leaf().to_owned());
                from_manifest(tombstone, context)
            },
            context,
        )?;
        assert_eq!(seen, ["generation"]);
        assert_eq!((report.removed, report.retained, report.preserved, report.busy), (1, 0, 0, 0));
        assert_eq!(entries(&folder)?, [".retired-not-a-tombstone"]);
        Ok(())
    })
}

#[test]
fn an_inventoried_file_opened_after_the_rename_is_retried_and_keeps_the_final_records() -> TestResult
{
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        generation(&folder)?;
        let name = super::tombstone::name("generation")?;
        std::fs::rename(folder.join("generation"), folder.join(&name))?;
        let reader = std::fs::File::open(folder.join(&name).join("app").join("one.dll"))?;
        let report = root.resume(&mut |tombstone| from_manifest(tombstone, context), context)?;
        assert_eq!((report.removed, report.retained, report.preserved, report.busy), (0, 1, 0, 1));
        assert!(folder.join(&name).join("manifest.json").exists());
        assert!(!folder.join(&name).join("app").join("nested").exists());
        drop(reader);
        let report = root.resume(&mut |tombstone| from_manifest(tombstone, context), context)?;
        assert_eq!((report.removed, report.retained, report.preserved, report.busy), (1, 0, 0, 0));
        assert!(entries(&folder)?.is_empty());
        Ok(())
    })
}

#[test]
fn cancellation_stops_retirement_before_any_change() -> TestResult {
    test_worker::run_reserved_expiring(16 * 1024 * 1024, |context, clock| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        generation(&folder)?;
        clock.expire();
        assert!(matches!(
            root.retire("generation", &RetirementUnit::Tree(inventory()?), context),
            Err(RetirementError::Stopped(_))
        ));
        assert_eq!(entries(&folder)?, ["generation"]);
        Ok(())
    })
}

#[test]
fn interrupted_stage_files_in_owned_folders_are_removed_and_lookalikes_are_kept() -> TestResult {
    test_worker::run(|context, _| {
        let (installation, folder) = owned(context)?;
        let root = owned_retirement_root(installation.path(), &folder, context)?;
        let generation = generation(&folder)?;
        std::fs::write(generation.join("app").join(".payload-stage-0A1B"), b"partial")?;
        std::fs::write(generation.join(".payload-stage-FFEE"), b"partial")?;
        std::fs::write(generation.join("app").join(".payload-stage-"), b"bare prefix")?;
        std::fs::write(generation.join("app").join("payload-stage-0A1B"), b"user")?;
        let inventory = inventory()?.stages(".payload-stage-")?;
        assert_eq!(
            root.retire("generation", &RetirementUnit::Tree(inventory), context)?.outcome,
            RetirementOutcome::Retained { preserved: 2, busy: 0 }
        );
        let names = entries(&folder)?;
        let tombstone = folder.join(&names[0]);
        assert_eq!(entries(&tombstone)?, ["app"]);
        assert_eq!(entries(&tombstone.join("app"))?, [".payload-stage-", "payload-stage-0A1B"]);
        for prefix in ["payload-stage-", ".stage", ".a/b-", ".payload-stage"] {
            assert!(matches!(
                TreeInventory::default().stages(prefix),
                Err(RetirementError::Invalid(_))
            ));
        }
        Ok(())
    })
}
