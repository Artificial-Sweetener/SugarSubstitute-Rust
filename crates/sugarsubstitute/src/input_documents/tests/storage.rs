// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Atomic saves, recovery, rejection evidence, cancellation and autosave on real execution
//! jobs over owned temporary directories.
use super::super::{files, *};
use super::{POLICY, at};
use crate::{
    completion_wake::{Gate, Wake},
    job_sequence::JobSequence,
};
use std::{
    error::Error, num::NonZeroUsize, os::windows::fs::OpenOptionsExt, path::PathBuf, sync::Arc,
};
use sugarsubstitute_canvas::{
    ArchivedDocument, Document, DocumentEdit, Layer, LayerContent, LayerId, MaskAppearance,
    SourceReference,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_image_ops::{AssetId, Coverage, Extent, FULL_COVERAGE, Region};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
#[cfg(feature = "native-composition")]
mod legacy;

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

struct NoRelease;
impl MemoryRelease for NoRelease {
    fn release(&self, _: u64) {}
}

struct Fixture {
    wake: Arc<Wake>,
    client: ExecutionClient,
    scope: Scope,
    jobs: JobSequence,
    memory: MemoryLease,
    store: InputDocumentStore,
    _directory: tempfile::TempDir,
    _owner: ExecutionOwner,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let directory = tempfile::tempdir()?;
        let store = InputDocumentStore::new(directory.path().join("session"))?;
        let memory = MemoryLease::new(64 * 1024 * 1024, Box::new(NoRelease));
        Ok(Self {
            wake,
            client,
            scope,
            jobs: JobSequence::new(),
            memory,
            store,
            _directory: directory,
            _owner: owner,
        })
    }

    fn spec(&self, bytes: u64) -> TestResult<JobSpec> {
        Ok(JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(60_000_000_000)),
            reservation_bytes: bytes,
            replacement: None,
        })
    }

    /// Wait for completion signals until `observe` reports a terminal result.
    fn settle<T>(&self, mut observe: impl FnMut() -> TestResult<Option<T>>) -> TestResult<T> {
        self.wake.until(|| {
            if let Err(cause) = self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))
            {
                return Some(Err(cause.into()));
            }
            observe().transpose()
        })
    }

    fn outcome<T, E>(&self, mut job: OwnedJob<T, E>) -> TestResult<JobOutcome<T, E>> {
        self.settle(|| Ok(job.try_take_completion().map(|completion| completion.outcome)))
    }

    fn save(&self, archive: &InputDocumentArchive) -> TestResult<Result<SaveReceipt, StoreError>> {
        let spec = self.spec(InputDocumentStore::save_reservation(archive))?;
        let job = self.store.submit_save(&self.client, &self.scope, spec, archive.clone())?;
        match self.outcome(job)? {
            JobOutcome::Success(receipt) => Ok(Ok(receipt)),
            JobOutcome::Failure(cause) => Ok(Err(cause)),
            other => Err(format!("Unexpected save outcome {:?}", other.kind()).into()),
        }
    }

    fn load(&self) -> TestResult<Restored> {
        let spec = self.spec(InputDocumentStore::load_reservation())?;
        let job = self.store.submit_load(&self.client, &self.scope, spec, self.memory.clone())?;
        match self.outcome(job)? {
            JobOutcome::Success(restored) => Ok(restored),
            JobOutcome::Failure(cause) => Err(cause.into()),
            other => Err(format!("Unexpected load outcome {:?}", other.kind()).into()),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.store.directory().join(name)
    }

    fn names(&self) -> TestResult<Vec<String>> {
        let mut names = std::fs::read_dir(self.store.directory())?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }

    /// One document with a locked base image, a painted mask and a selection; `shade`
    /// distinguishes otherwise identical content.
    fn archive(&self, shade: u16, metadata: &[u8]) -> TestResult<InputDocumentArchive> {
        self.archive_named("workflow/7", shade, metadata)
    }
    fn archive_named(
        &self,
        key: &str,
        shade: u16,
        metadata: &[u8],
    ) -> TestResult<InputDocumentArchive> {
        let extent = Extent::new(300, 200)?;
        let mut document = Document::new(1, extent, &self.memory)?;
        let base =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([9; 32]), extent })?
                .with_locked(true);
        let mut coverage = Coverage::new(extent, 0, &self.memory)?;
        coverage.update(Region::new(100, 50, 90, 70)?, &self.memory, |x, y, _| {
            shade.wrapping_add((x * 13 + y) as u16)
        })?;
        let mask = Layer::new(
            LayerId::new(2)?,
            LayerContent::Coverage { coverage, appearance: MaskAppearance::new([255, 0, 0], 0.5)? },
        )?;
        document.apply(DocumentEdit::Insert { index: 0, layer: base })?;
        document.apply(DocumentEdit::Insert { index: 1, layer: mask })?;
        let selection = Coverage::new(extent, FULL_COVERAGE, &self.memory)?;
        document.apply(DocumentEdit::Select(Some(selection)))?;
        let reference = SourceReference::new(br#"{"filename":"base.png","type":"input"}"#)?;
        let document =
            ArchivedDocument::capture(key, &document.snapshot(), |_| Some(reference.clone()))?;
        Ok(InputDocumentArchive::new(vec![document], metadata)?)
    }
}

fn restored(restored: &Restored) -> Option<(ArchiveFile, &InputDocumentArchive)> {
    restored.archive.as_ref().map(|(file, archive)| (*file, archive))
}

#[test]
fn saves_replace_the_archive_atomically_and_keep_the_replaced_one_for_recovery() -> TestResult {
    let fixture = Fixture::new()?;
    let empty = fixture.load()?;
    assert!(empty.archive.is_none() && empty.rejected.is_empty(), "a new session is a miss");
    let first = fixture.archive(1, b"first")?;
    let receipt = fixture.save(&first)??;
    assert_eq!((receipt.kept_previous, receipt.evidence.len()), (false, 0));
    assert_eq!(fixture.names()?, [CURRENT_ARCHIVE]);
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &first)));
    let first_bytes = std::fs::read(fixture.path(CURRENT_ARCHIVE))?;
    assert_eq!(receipt.bytes, first_bytes.len() as u64);
    let second = fixture.archive(2, b"second")?;
    assert!(fixture.save(&second)??.kept_previous);
    assert_eq!(std::fs::read(fixture.path(PREVIOUS_ARCHIVE))?, first_bytes);
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &second)));
    Ok(())
}

#[test]
fn a_rejected_archive_restores_the_recovery_copy_and_is_preserved_before_replacement() -> TestResult
{
    let fixture = Fixture::new()?;
    let first = fixture.archive(1, b"first")?;
    fixture.save(&first)??;
    fixture.save(&fixture.archive(2, b"second")?)??;
    let previous = std::fs::read(fixture.path(PREVIOUS_ARCHIVE))?;
    let mut damaged = std::fs::read(fixture.path(CURRENT_ARCHIVE))?;
    let middle = damaged.len() / 2;
    damaged[middle] ^= 0x40;
    std::fs::write(fixture.path(CURRENT_ARCHIVE), &damaged)?;
    std::fs::write(fixture.path("notes.txt"), b"unknown")?;
    let loaded = fixture.load()?;
    assert_eq!(restored(&loaded), Some((ArchiveFile::Previous, &first)));
    assert_eq!(loaded.rejected.len(), 1);
    assert_eq!(loaded.rejected[0].file, ArchiveFile::Current);
    assert!(matches!(loaded.rejected[0].cause, ArchiveError::Corrupt(_)));
    let third = fixture.archive(3, b"third")?;
    let receipt = fixture.save(&third)??;
    assert!(!receipt.kept_previous, "a rejected archive never becomes the recovery copy");
    assert_eq!(receipt.evidence.len(), 1);
    assert_eq!(std::fs::read(&receipt.evidence[0])?, damaged);
    assert_eq!(std::fs::read(fixture.path(PREVIOUS_ARCHIVE))?, previous);
    assert_eq!(std::fs::read(fixture.path("notes.txt"))?, b"unknown");
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &third)));
    assert_eq!(fixture.names()?.len(), 4, "current, recovery, evidence and the unknown file");
    Ok(())
}

#[test]
fn unsupported_and_truncated_archives_are_observable_misses() -> TestResult {
    let fixture = Fixture::new()?;
    fixture.save(&fixture.archive(1, b"first")?)??;
    let bytes = std::fs::read(fixture.path(CURRENT_ARCHIVE))?;
    let mut future = bytes.clone();
    let unsupported = sugarsubstitute_canvas::INPUT_ARCHIVE_VERSION + 1;
    future[8..12].copy_from_slice(&unsupported.to_le_bytes());
    std::fs::write(fixture.path(CURRENT_ARCHIVE), &future)?;
    let loaded = fixture.load()?;
    assert!(loaded.archive.is_none());
    assert!(matches!(
        loaded.rejected[0].cause,
        ArchiveError::UnsupportedVersion(version) if version == unsupported
    ));
    std::fs::write(fixture.path(CURRENT_ARCHIVE), &bytes[..bytes.len() - 9])?;
    let loaded = fixture.load()?;
    assert!(loaded.archive.is_none());
    assert!(matches!(loaded.rejected[0].cause, ArchiveError::Truncated));
    assert_eq!(fixture.names()?, [CURRENT_ARCHIVE], "rejection alone changes no file");
    Ok(())
}

#[test]
fn a_failed_replacement_keeps_the_current_archive_intact() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.archive(1, b"first")?;
    fixture.save(&first)??;
    let before = std::fs::read(fixture.path(CURRENT_ARCHIVE))?;
    // Read sharing without delete sharing makes the final atomic replacement fail.
    let held =
        std::fs::OpenOptions::new().read(true).share_mode(1).open(fixture.path(CURRENT_ARCHIVE))?;
    let failed = fixture.save(&fixture.archive(2, b"second")?)?;
    assert!(matches!(failed, Err(StoreError::Storage(_))), "{failed:?}");
    drop(held);
    assert_eq!(std::fs::read(fixture.path(CURRENT_ARCHIVE))?, before);
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &first)));
    Ok(())
}

#[test]
fn a_cancelled_save_changes_no_file() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.archive(1, b"first")?;
    fixture.save(&first)??;
    let before = std::fs::read(fixture.path(CURRENT_ARCHIVE))?;
    let second = fixture.archive(2, b"second")?;
    let gate = Arc::new(Gate::default());
    let worker_gate = gate.clone();
    let directory = fixture.store.directory().to_path_buf();
    let spec = fixture.spec(InputDocumentStore::save_reservation(&second))?;
    let job = fixture.client.submit_blocking(&fixture.scope, spec, move |context| {
        worker_gate.hold();
        files::save(&directory, &second, &context)
    })?;
    job.cancel();
    gate.release();
    match fixture.outcome(job)? {
        JobOutcome::Cancelled => {}
        JobOutcome::Failure(cause) => assert!(cause.interrupted(), "{cause}"),
        other => return Err(format!("Unexpected cancelled outcome {:?}", other.kind()).into()),
    }
    assert_eq!(std::fs::read(fixture.path(CURRENT_ARCHIVE))?, before);
    assert_eq!(fixture.names()?, [CURRENT_ARCHIVE]);
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &first)));
    Ok(())
}

#[test]
fn autosave_coalesces_changes_and_skips_content_already_on_disk() -> TestResult {
    let fixture = Fixture::new()?;
    let mut autosave = Autosave::new(fixture.store.clone(), POLICY, 0, None);
    let first = fixture.archive(1, b"first")?;
    autosave.changed(1, at(0))?;
    autosave.changed(2, at(100))?;
    let unexpected = || -> Result<InputDocumentArchive, ArchiveError> {
        Err(ArchiveError::Invalid("Captured before the quiet period ended"))
    };
    assert!(
        autosave
            .poll(at(200), &fixture.client, &fixture.scope, &fixture.jobs, unexpected)?
            .is_none()
    );
    assert!(!autosave.saving());
    assert_eq!(autosave.deadline(), Some(at(600)));
    let captured = first.clone();
    autosave.poll(at(600), &fixture.client, &fixture.scope, &fixture.jobs, || Ok(captured))?;
    assert!(autosave.saving());
    let outcome = fixture.settle(|| Ok(autosave.observe(at(650))?))?;
    assert!(matches!(outcome, SaveOutcome::Saved(SaveReceipt { kept_previous: false, .. })));
    assert!(!autosave.dirty());
    autosave.changed(3, at(700))?;
    let same = first.clone();
    let unchanged =
        autosave.poll(at(1_200), &fixture.client, &fixture.scope, &fixture.jobs, || Ok(same))?;
    assert!(matches!(unchanged, Some(SaveOutcome::Unchanged)));
    assert!(!autosave.saving() && !autosave.dirty());
    autosave.changed(4, at(1_300))?;
    let second = fixture.archive(2, b"second")?;
    let captured = second.clone();
    autosave.poll(at(1_800), &fixture.client, &fixture.scope, &fixture.jobs, || Ok(captured))?;
    let outcome = fixture.settle(|| Ok(autosave.observe(at(1_850))?))?;
    assert!(matches!(outcome, SaveOutcome::Saved(SaveReceipt { kept_previous: true, .. })));
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &second)));
    Ok(())
}

#[test]
fn a_close_flush_saves_at_once_and_one_past_its_cutoff_keeps_the_previous_archive() -> TestResult {
    let fixture = Fixture::new()?;
    let mut autosave = Autosave::new(fixture.store.clone(), POLICY, 0, None);
    let first = fixture.archive(1, b"first")?;
    autosave.changed(1, at(0))?;
    assert_eq!(autosave.deadline(), Some(at(500)), "the debounce has not elapsed");
    let captured = first.clone();
    let cutoff = Moment::from_nanos(60_000_000_000);
    let started =
        autosave.flush(at(0), cutoff, &fixture.client, &fixture.scope, &fixture.jobs, || {
            Ok(captured)
        })?;
    assert!(started.is_none() && autosave.saving(), "the flush ignores the debounce");
    let outcome = fixture.settle(|| Ok(autosave.observe(at(0))?))?;
    assert!(matches!(outcome, SaveOutcome::Saved(_)));
    assert!(!autosave.dirty());
    // A final save whose shutdown cutoff has passed never replaces the archive on disk.
    autosave.changed(2, at(0))?;
    let second = fixture.archive(2, b"second")?;
    let late =
        autosave.flush(at(0), at(0), &fixture.client, &fixture.scope, &fixture.jobs, || Ok(second));
    match late {
        Err(AutosaveError::Admission(_)) => {}
        Ok(None) if autosave.saving() => {
            let outcome = fixture.settle(|| Ok(autosave.observe(at(0))?))?;
            assert!(matches!(outcome, SaveOutcome::Stopped(_) | SaveOutcome::Failed(_)));
        }
        other => return Err(format!("A late flush started unexpectedly: {other:?}").into()),
    }
    assert!(autosave.dirty(), "the unsaved change stays unsaved");
    assert_eq!(restored(&fixture.load()?), Some((ArchiveFile::Current, &first)));
    Ok(())
}

#[test]
fn missing_current_input_archive_recovers_verified_previous_without_rewriting_originals()
-> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.archive(1, b"first")?;
    fixture.save(&first)??;
    fixture.save(&fixture.archive(2, b"second")?)??;
    let previous = std::fs::read(fixture.path(PREVIOUS_ARCHIVE))?;
    std::fs::remove_file(fixture.path(CURRENT_ARCHIVE))?;
    let loaded = fixture.load()?;
    let (file, archive) =
        loaded.archive.as_ref().ok_or("Verified previous archive was not restored")?;
    assert_eq!(*file, ArchiveFile::Previous);
    assert_eq!(archive.fingerprint(), first.fingerprint());
    assert!(archive == &first);
    assert_eq!(std::fs::read(fixture.path(PREVIOUS_ARCHIVE))?, previous);
    assert!(!fixture.path(CURRENT_ARCHIVE).exists());
    Ok(())
}
