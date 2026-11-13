// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify which generations a turn keeps and removes on real NTFS folders and records.
use super::*;
use crate::{
    durable_storage::{files::reparse_fixture, protected::directories::Mode},
    setup_storage::{
        InstallationRun, OwnedTrial,
        test_worker::{self, TestResult},
    },
};
use std::{path::Path, sync::Mutex};
use sugarsubstitute_contracts::installation::{
    BadTrial, Generation, PackageDigest, PreparationIntent, SignedRelease, TrialVerdict,
};

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// Turn reservation plus the fixture's own record and folder work.
const RESERVATION: u64 = GENERATION_RETIREMENT_RESERVATION_BYTES + 16 * 1024 * 1024;

/// A launch pointer whose answer the test chooses.
struct Pointer(Mutex<Option<Vec<GenerationId>>>);
impl Pointer {
    fn naming(generations: &[&Generation]) -> Self {
        Self(Mutex::new(Some(generations.iter().map(|g| g.id.clone()).collect())))
    }
    fn damaged() -> Self {
        Self(Mutex::new(None))
    }
}
impl PointedGenerations for Pointer {
    fn pointed_generations(
        &self,
        _installation: &Path,
        _context: &JobContext,
    ) -> Result<Vec<GenerationId>, PointerFailure> {
        crate::native_claims::lock(&self.0).clone().ok_or(PointerFailure::Damaged)
    }
}

/// An owned installation, its selection store and an empty generation folder.
struct Installation {
    root: tempfile::TempDir,
    store: Arc<SelectionStore>,
    record: InstallationRecord,
}

impl Installation {
    fn new(context: &JobContext) -> Result<Self, Failure> {
        let root = test_worker::installation(7, context)?;
        std::fs::create_dir(root.path().join("releases"))?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let record = store.load(context)?.ok_or("record")?;
        Ok(Self { root, store, record })
    }

    fn releases(&self) -> PathBuf {
        self.root.path().join("releases")
    }

    /// Prepare `generation` as the candidate, superseding any earlier candidate.
    fn prepare(&mut self, generation: &Generation, context: &JobContext) -> Result<(), Failure> {
        let now = super::super::selection::unix_now()?;
        let staging = match self.record.candidate() {
            None => self.record.prepare(generation.clone(), PreparationIntent::Automatic, now)?,
            Some(previous) => self.record.supersede_preparation(
                previous.generation(),
                generation.clone(),
                PreparationIntent::Automatic,
                now,
            )?,
        };
        self.store.compare_exchange(&self.record, &staging, context)?;
        let prepared = staging.prepared(generation)?;
        self.store.compare_exchange(&staging, &prepared, context)?;
        self.record = prepared;
        Ok(())
    }

    /// Start the prepared candidate's trial and settle it with `verdict`, or accept it.
    fn settle(
        &mut self,
        verdict: Option<TrialVerdict>,
        context: &JobContext,
    ) -> Result<(), Failure> {
        let mut run = InstallationRun::acquire(self.store.clone(), context)?;
        let trial: OwnedTrial = run.begin_trial(&self.record, context)?;
        self.record = match verdict {
            None => run.accept(&trial, context)?,
            Some(verdict) => run.finish_trial(&trial, verdict, context)?,
        };
        Ok(())
    }

    /// One turn under run custody, as the coordinator runs it.
    fn turn(
        &self,
        pointer: &Pointer,
        context: &JobContext,
    ) -> Result<GenerationTurn, GenerationRetirementError> {
        let _run = InstallationRun::acquire(self.store.clone(), context)
            .map_err(GenerationRetirementError::Selection)?;
        retire_held(self.root.path(), None, pointer, context)
    }

    fn finished(
        &self,
        pointer: &Pointer,
        context: &JobContext,
    ) -> Result<GenerationReport, Failure> {
        match self.turn(pointer, context)? {
            GenerationTurn::Finished(report) => Ok(report),
            other => Err(format!("turn did not finish: {other:?}").into()),
        }
    }

    fn names(&self) -> std::io::Result<Vec<String>> {
        let mut names = std::fs::read_dir(self.releases())?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }
}

fn generation(number: u64) -> Result<Generation, Failure> {
    Ok(Generation {
        id: GenerationId::new(format!("{number:032x}"))?,
        package: PackageDigest::new(format!("{number:064x}"))?,
        release: SignedRelease::new(format!("1.0.{number}"), number)?,
    })
}

/// Write a complete generation folder: package files, inventory and installation records.
/// Returns the bytes it wrote.
fn write_generation(releases: &Path, generation: &Generation) -> Result<u64, Failure> {
    let folder = releases.join(generation.id.as_str());
    std::fs::create_dir(&folder)?;
    let (manifest, _) = super::super::test_package::distribution()?;
    let mut bytes = 0;
    for entry in Manifest::decode(&manifest)?.entries() {
        let Some(relative) = entry.path().strip_prefix("app/") else { continue };
        let path = folder.join(relative);
        std::fs::create_dir_all(path.parent().ok_or("parent")?)?;
        let content = format!("inert fixture payload: {}", entry.path());
        std::fs::write(&path, &content)?;
        bytes += content.len() as u64;
    }
    for (name, content) in [
        (INSTALLED_MANIFEST, manifest.as_slice()),
        ("setup-generation.json", br#"{"schema_version":1}"#.as_slice()),
        ("installation-ready.json", br#"{"schema_version":1}"#.as_slice()),
    ] {
        std::fs::write(folder.join(name), content)?;
        bytes += content.len() as u64;
    }
    Ok(bytes)
}

fn folder_bytes(path: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        total += if kind.is_dir() { folder_bytes(&entry.path())? } else { entry.metadata()?.len() };
    }
    Ok(total)
}

use super::super::inventory::INSTALLED_MANIFEST;
use sugarsubstitute_update::package::Manifest;

/// Accepted `1`, retained none: the starting point most tests build on.
fn accepted_one(context: &JobContext) -> Result<(Installation, Generation), Failure> {
    let mut installation = Installation::new(context)?;
    let first = generation(1)?;
    write_generation(&installation.releases(), &first)?;
    installation.prepare(&first, context)?;
    installation.settle(None, context)?;
    Ok((installation, first))
}

#[test]
fn a_superseded_candidate_is_removed_and_the_new_candidate_kept() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let (superseded, candidate) = (generation(2)?, generation(3)?);
        for generation in [&superseded, &candidate] {
            write_generation(&installation.releases(), generation)?;
            installation.prepare(generation, context)?;
        }
        let report = installation.finished(&Pointer::naming(&[&first]), context)?;
        assert_eq!((report.removed, report.in_use, report.retained), (1, 0, 0));
        assert!(report.freed_bytes > 0);
        assert_eq!(installation.names()?, [first.id.as_str(), candidate.id.as_str()]);
        Ok(())
    })
}

#[test]
fn a_rejected_trial_is_removed_and_the_accepted_version_kept() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let rejected = generation(2)?;
        write_generation(&installation.releases(), &rejected)?;
        installation.prepare(&rejected, context)?;
        installation.settle(Some(TrialVerdict::Bad(BadTrial::Crashed)), context)?;
        assert_eq!(installation.record.rollback(), Some(&rejected));
        let report = installation.finished(&Pointer::naming(&[&first]), context)?;
        assert_eq!(report.removed, 1);
        assert_eq!(installation.names()?, [first.id.as_str()]);
        Ok(())
    })
}

#[test]
fn an_older_previous_version_is_removed_and_accepted_and_retained_kept() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let (second, third) = (generation(2)?, generation(3)?);
        for generation in [&second, &third] {
            write_generation(&installation.releases(), generation)?;
            installation.prepare(generation, context)?;
            installation.settle(None, context)?;
        }
        assert_eq!(installation.record.retained(), Some(&second));
        let report = installation.finished(&Pointer::naming(&[&third, &second]), context)?;
        assert_eq!(report.removed, 1);
        assert_eq!(installation.names()?, [second.id.as_str(), third.id.as_str()]);
        assert!(!installation.releases().join(first.id.as_str()).exists());
        Ok(())
    })
}

#[test]
fn every_generation_the_launch_pointer_names_is_kept() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let (second, third) = (generation(2)?, generation(3)?);
        for generation in [&second, &third] {
            write_generation(&installation.releases(), generation)?;
            installation.prepare(generation, context)?;
            installation.settle(None, context)?;
        }
        // The pointer still falls back to the oldest version, which the record has dropped.
        let report = installation.finished(&Pointer::naming(&[&third, &first]), context)?;
        assert_eq!(report.removed, 0);
        assert_eq!(installation.names()?.len(), 3);
        // An unreadable pointer keeps every version and changes nothing.
        assert!(matches!(
            installation.turn(&Pointer::damaged(), context)?,
            GenerationTurn::PointerUnreadable(PointerFailure::Damaged)
        ));
        assert_eq!(installation.names()?.len(), 3);
        Ok(())
    })
}

#[test]
fn a_crash_between_the_tombstone_rename_and_deletion_resumes_on_the_next_turn() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let (second, third) = (generation(2)?, generation(3)?);
        for generation in [&second, &third] {
            write_generation(&installation.releases(), generation)?;
            installation.prepare(generation, context)?;
            installation.settle(None, context)?;
        }
        // The crash: the rename committed the retirement and part of the deletion happened.
        let tombstone = format!(".retired-{:032x}.{}", 0xfeed_u64, first.id.as_str());
        let releases = installation.releases();
        std::fs::rename(releases.join(first.id.as_str()), releases.join(&tombstone))?;
        std::fs::remove_file(releases.join(&tombstone).join("sugarsubstitute.exe"))?;
        let report = installation.finished(&Pointer::naming(&[&third, &second]), context)?;
        assert_eq!((report.removed, report.retained, report.preserved), (1, 0, 0));
        assert_eq!(installation.names()?, [second.id.as_str(), third.id.as_str()]);
        Ok(())
    })
}

#[test]
fn a_generation_in_use_survives_and_is_removed_once_released() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let (second, third) = (generation(2)?, generation(3)?);
        for generation in [&second, &third] {
            write_generation(&installation.releases(), generation)?;
            installation.prepare(generation, context)?;
            installation.settle(None, context)?;
        }
        // A previous copy still running holds its executable open.
        let running = std::fs::File::open(
            installation.releases().join(first.id.as_str()).join("sugarsubstitute.exe"),
        )?;
        let pointer = Pointer::naming(&[&third, &second]);
        let report = installation.finished(&pointer, context)?;
        assert_eq!((report.removed, report.in_use), (0, 1));
        assert_eq!(installation.names()?.len(), 3);
        assert!(
            installation.releases().join(first.id.as_str()).join("sugarsubstitute.exe").is_file()
        );
        drop(running);
        assert_eq!(installation.finished(&pointer, context)?.removed, 1);
        assert_eq!(installation.names()?, [second.id.as_str(), third.id.as_str()]);
        Ok(())
    })
}

#[test]
fn unknown_files_are_preserved_and_unknown_folder_entries_untouched() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let second = generation(2)?;
        write_generation(&installation.releases(), &second)?;
        installation.prepare(&second, context)?;
        installation.settle(None, context)?;
        let third = generation(3)?;
        write_generation(&installation.releases(), &third)?;
        installation.prepare(&third, context)?;
        installation.settle(None, context)?;
        let releases = installation.releases();
        std::fs::write(releases.join(first.id.as_str()).join("notes.txt"), b"mine")?;
        std::fs::write(releases.join("readme.txt"), b"also mine")?;
        std::fs::create_dir(releases.join("Backup"))?;
        let report = installation.finished(&Pointer::naming(&[&third]), context)?;
        assert_eq!((report.removed, report.retained, report.preserved), (0, 1, 1));
        assert_eq!(report.unknown, 2);
        let names = installation.names()?;
        let tombstone =
            names.iter().find(|name| name.starts_with(".retired-")).ok_or("tombstone")?;
        assert_eq!(std::fs::read(releases.join(tombstone).join("notes.txt"))?, b"mine");
        assert!(!releases.join(tombstone).join("sugarsubstitute.exe").exists());
        assert!(!releases.join(tombstone).join(INSTALLED_MANIFEST).exists());
        assert_eq!(std::fs::read(releases.join("readme.txt"))?, b"also mine");
        assert!(releases.join("Backup").is_dir());
        // A later turn keeps the preserved file and reports it again.
        let again = installation.finished(&Pointer::naming(&[&third]), context)?;
        assert_eq!((again.retained, again.preserved), (1, 1));
        Ok(())
    })
}

#[test]
fn a_junction_inside_a_generation_is_kept_and_never_followed() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        for number in [2, 3] {
            let generation = generation(number)?;
            write_generation(&installation.releases(), &generation)?;
            installation.prepare(&generation, context)?;
            installation.settle(None, context)?;
        }
        let outside = tempfile::tempdir()?;
        std::fs::write(outside.path().join("sugarsubstitute.exe"), b"elsewhere")?;
        let old = installation.releases().join(first.id.as_str());
        reparse_fixture::junction(&old.join("linked"), outside.path(), context)?;
        let report = installation.finished(&Pointer::naming(&[]), context)?;
        assert_eq!((report.retained, report.preserved), (1, 1));
        assert_eq!(std::fs::read(outside.path().join("sugarsubstitute.exe"))?, b"elsewhere");
        // A junction in a generation's own place is never entered either.
        let fourth = installation.releases().join(format!("{:032x}", 4));
        reparse_fixture::junction(&fourth, outside.path(), context)?;
        let report = installation.finished(&Pointer::naming(&[]), context)?;
        assert_eq!(report.removed, 0);
        assert_eq!(std::fs::read(outside.path().join("sugarsubstitute.exe"))?, b"elsewhere");
        Ok(())
    })
}

#[test]
fn a_running_preparation_skips_the_turn_and_a_staging_candidate_is_kept() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let superseded = generation(2)?;
        write_generation(&installation.releases(), &superseded)?;
        installation.prepare(&superseded, context)?;
        let trust = ownership::state_path(&Directory::open(installation.root.path(), false)?)
            .join(super::super::trust::DIRECTORY);
        protection::with(context, |protection| {
            protection.directory(&trust, Mode::OpenOrCreate, context).map(|_| ())
        })?;
        // The preparation has reserved a new candidate and is creating its folder.
        let held =
            match try_preparation(&Directory::open(installation.root.path(), false)?, context)? {
                PreparationAttempt::Held(file) => file,
                _ => return Err("preparation was not taken".into()),
            };
        let staging = generation(3)?;
        let now = super::super::selection::unix_now()?;
        let reserved = installation.record.supersede_preparation(
            &superseded,
            staging.clone(),
            PreparationIntent::Automatic,
            now,
        )?;
        installation.store.compare_exchange(&installation.record, &reserved, context)?;
        installation.record = reserved;
        std::fs::create_dir(installation.releases().join(staging.id.as_str()))?;
        std::fs::write(
            installation.releases().join(staging.id.as_str()).join("setup-generation.json"),
            b"{}",
        )?;
        assert!(matches!(
            installation.turn(&Pointer::naming(&[&first]), context)?,
            GenerationTurn::PreparationBusy
        ));
        assert_eq!(installation.names()?.len(), 3);
        drop(held);
        // Once preparation ends, the staging candidate the record names stays; the
        // superseded candidate goes.
        let report = installation.finished(&Pointer::naming(&[&first]), context)?;
        assert_eq!(report.removed, 1);
        assert_eq!(installation.names()?, [first.id.as_str(), staging.id.as_str()]);
        // A turn holding preparation keeps any preparation from starting meanwhile.
        let _run = InstallationRun::acquire(installation.store.clone(), context)?;
        let pinned = Directory::open(installation.root.path(), false)?;
        let mine = try_preparation(&pinned, context)?;
        assert!(matches!(mine, PreparationAttempt::Held(_)));
        assert!(matches!(try_preparation(&pinned, context)?, PreparationAttempt::Busy));
        Ok(())
    })
}

#[test]
fn an_interrupted_generation_creation_is_removed_and_its_marker_last() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (installation, first) = accepted_one(context)?;
        let stage = installation.releases().join(".generation-init-0A1B2C3D");
        std::fs::create_dir(&stage)?;
        std::fs::write(stage.join("setup-generation.json"), b"{}")?;
        let report = installation.finished(&Pointer::naming(&[&first]), context)?;
        assert_eq!(report.removed, 1);
        assert_eq!(installation.names()?, [first.id.as_str()]);
        Ok(())
    })
}

#[test]
fn disk_use_stays_flat_across_ten_updates() -> TestResult {
    test_worker::run_reserved(RESERVATION, |context, _| {
        let (mut installation, first) = accepted_one(context)?;
        let mut previous = first;
        let mut largest = 0;
        for number in 2..=11 {
            let next = generation(number)?;
            largest = largest.max(write_generation(&installation.releases(), &next)?);
            installation.prepare(&next, context)?;
            installation.settle(None, context)?;
            installation.finished(&Pointer::naming(&[&next, &previous]), context)?;
            assert_eq!(installation.names()?, [previous.id.as_str(), next.id.as_str()]);
            assert!(folder_bytes(&installation.releases())? <= 2 * largest);
            previous = next;
        }
        Ok(())
    })
}

#[test]
fn cancellation_stops_a_turn_before_any_change() -> TestResult {
    test_worker::run_reserved_expiring(RESERVATION, |context, clock| {
        let (mut installation, _) = accepted_one(context)?;
        for number in [2, 3] {
            let generation = generation(number)?;
            write_generation(&installation.releases(), &generation)?;
            installation.prepare(&generation, context)?;
            installation.settle(None, context)?;
        }
        let _run = InstallationRun::acquire(installation.store.clone(), context)?;
        clock.expire();
        assert!(matches!(
            retire_held(installation.root.path(), None, &Pointer::naming(&[]), context),
            Err(GenerationRetirementError::Stopped(_))
        ));
        assert_eq!(installation.names()?.len(), 3);
        Ok(())
    })
}

#[test]
fn the_running_generation_and_the_installation_folder_are_read_from_its_path() -> TestResult {
    let installation = Path::new(r"C:\SugarSubstitute");
    let running = installation.join("releases").join(format!("{:032x}", 1));
    assert_eq!(installation_of(&running)?, installation);
    assert!(installation_of(&installation.join("other").join("x")).is_err());
    let kept = keep_set(
        &InstallationRecord::empty(sugarsubstitute_contracts::installation::InstallationId::new(
            "1".repeat(32),
        )?),
        vec![generation(4)?.id],
        Some(generation(5)?.id),
        Some(&generation(6)?.id),
    );
    assert_eq!(kept.len(), 3);
    assert!(matches!(classify(&format!("{:032x}", 4), &kept), Entry::Kept));
    assert!(matches!(classify(&format!("{:032x}", 7), &kept), Entry::Generation));
    assert!(matches!(classify(&format!("{:032X}", 0xab), &kept), Entry::Unknown));
    assert!(matches!(classify(".retired-x", &kept), Entry::Tombstone));
    assert!(matches!(classify(".generation-init-1", &kept), Entry::Initializing));
    assert!(matches!(classify(".generation-init-", &kept), Entry::Unknown));
    Ok(())
}

#[test]
fn the_production_pointer_keeps_exactly_what_the_launch_pointer_owner_names() -> TestResult {
    use crate::setup_storage::launch_pointer_fixture::{engine, point};
    test_worker::run_reserved(4 * crate::setup_storage::POINTER_RESERVATION_BYTES, |context, _| {
        let root = test_worker::installation(171, context)?;
        let production = LaunchPointerGenerations;
        assert!(production.pointed_generations(root.path(), context)?.is_empty());
        let first = GenerationId::new("1".repeat(32))?;
        let second = GenerationId::new("2".repeat(32))?;
        let primary = engine(root.path(), &first, b"first engine", context)?;
        let fallback = engine(root.path(), &second, b"second engine", context)?;
        point(
            root.path(),
            &crate::setup_storage::LaunchPointer::new(primary.clone(), Some(fallback)),
            context,
        )?;
        let named = production.pointed_generations(root.path(), context)?;
        assert_eq!(named, [first.clone(), second]);
        assert_eq!(named, crate::setup_storage::pointed_generations(root.path(), context)?);
        point(
            root.path(),
            &crate::setup_storage::LaunchPointer::new(primary.clone(), Some(primary)),
            context,
        )?;
        assert_eq!(production.pointed_generations(root.path(), context)?, [first]);
        Ok(())
    })
}
