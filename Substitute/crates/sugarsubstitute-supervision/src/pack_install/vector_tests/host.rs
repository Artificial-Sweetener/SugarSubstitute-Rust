// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Host behaviour beyond the shared vectors: a fault at every pack effect, and trial endings
//! only the host can witness.

use super::decisions::{install_registry_pack, pack_files};
use super::fixture::{exists, file_members, folder_files, zip_bytes};
use super::*;
use std::path::PathBuf;

/// The 1.1.0 release: one file dropped, one added, the rest changed.
fn release() -> BTreeMap<String, String> {
    let mut release = pack_files("1.1.0");
    release.remove("sugarcubes/core.py");
    release.insert("lib/added.py".into(), "added\n".into());
    release
}

/// An installed 1.0.0 with user data, and the extracted 1.1.0 release beside it.
struct Update {
    root: tempfile::TempDir,
    original: BTreeMap<String, String>,
    new_entries: Vec<EntryName>,
}

impl Update {
    fn new() -> std::result::Result<Self, Failure> {
        let root = tempfile::tempdir()?;
        let pack = install_registry_pack(root.path(), &pack_files("1.0.0"))?;
        std::fs::write(pack.join("user-notes.txt"), "kept")?;
        let staged = PackTree::at(&root.path().join("staged"))?;
        let archive = zip_bytes(&file_members(&release()))?;
        let new_entries =
            archive::extract_registry_archive(&archive, &staged, ArchiveLimits::default())?.entries;
        let original = folder_files(&pack)?;
        Ok(Self { root, original, new_entries })
    }

    fn pack(&self) -> PathBuf {
        self.root.path().join("custom_nodes").join("SugarCubes")
    }

    fn transaction_root(&self) -> PathBuf {
        self.root.path().join("transaction")
    }

    fn apply(&self, transaction: &replacement::TrackedReplacement) -> Result<()> {
        let target = PackTree::at(&self.pack())?;
        let staged = PackTree::at(&self.root.path().join("staged"))?;
        let ownership::PackOwnership::Owned(owned) = ownership::inspect_pack_folder(&target)?
        else {
            return Err(PackInstallError::format(ErrorCode::TargetChanged, "fixture is not owned"));
        };
        transaction.apply(&replacement::ReplacementPlan {
            target: &target,
            source: &staged,
            old_entries: &owned.entries,
            new_entries: &self.new_entries,
            from_version: "1.0.0",
            to_version: "1.1.0",
        })
    }
}
#[test]
fn a_failure_at_every_pack_effect_restores_the_exact_previous_folder() -> TestResult {
    let clean = Update::new()?;
    let counted = replacement::TrackedReplacement::with_faults(
        PackTree::at(&clean.transaction_root())?,
        None,
        None,
    );
    clean.apply(&counted)?;
    let effects = counted.effects();
    assert!(effects > 5);
    for fail_at in 1..=effects {
        let update = Update::new()?;
        let transaction = replacement::TrackedReplacement::with_faults(
            PackTree::at(&update.transaction_root())?,
            Some(fail_at),
            None,
        );
        assert!(update.apply(&transaction).is_err(), "effect {fail_at} did not fail");
        assert_eq!(folder_files(&update.pack())?, update.original, "effect {fail_at}");
        assert!(!exists(&update.transaction_root()), "effect {fail_at} left its transaction");
    }
    Ok(())
}

#[test]
fn a_power_cut_in_each_phase_is_recovered_at_the_next_start() -> TestResult {
    for phase in [JournalPhase::BackingUp, JournalPhase::Replacing] {
        let update = Update::new()?;
        let cut = replacement::TrackedReplacement::with_faults(
            PackTree::at(&update.transaction_root())?,
            None,
            Some(phase),
        );
        assert!(update.apply(&cut).is_err());
        let next = replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
        next.settle(&PackTree::at(&update.pack())?, TrialEnding::Unwitnessed)?;
        assert_eq!(folder_files(&update.pack())?, update.original, "{phase:?}");
        assert!(!exists(&update.transaction_root()), "{phase:?}");
    }
    Ok(())
}

#[test]
fn unwitnessed_endings_keep_the_update_until_the_limit_counts_as_a_failed_start() -> TestResult {
    let update = Update::new()?;
    let transaction =
        replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
    update.apply(&transaction)?;
    let target = PackTree::at(&update.pack())?;
    for _ in 1..UNWITNESSED_ENDINGS_LIMIT {
        assert_eq!(transaction.settle(&target, TrialEnding::Unwitnessed)?, None);
        assert_eq!(ownership::read_identity(&target)?.version.to_string(), "1.1.0");
    }
    let restored = transaction.settle(&target, TrialEnding::Unwitnessed)?.ok_or("not restored")?;
    assert_eq!(restored.reason, RestoreReason::DidNotStart);
    assert_eq!(folder_files(&update.pack())?, update.original);
    Ok(())
}

/// A page that did not load for an unattributed cause keeps the version for a later page of
/// the same run; with none, the next start restores the previous version, so the version
/// whose page never loaded does not run a second time.
#[test]
fn an_unconfirmed_page_restores_at_the_next_start_unless_a_later_page_loads() -> TestResult {
    let update = Update::new()?;
    let transaction =
        replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
    update.apply(&transaction)?;
    let target = PackTree::at(&update.pack())?;
    transaction.observe("1.1.0", TrialEvent::BackendStartUnobservable)?;
    transaction.observe("1.1.0", TrialEvent::PageUnconfirmed)?;
    let phase = transaction.pending()?.map(|pending| pending.phase);
    assert_eq!(phase, Some(JournalPhase::PageUnconfirmed));
    assert_eq!(transaction.settle(&target, TrialEnding::BackendStopped)?, None);
    let restored = transaction.settle(&target, TrialEnding::Unwitnessed)?.ok_or("kept")?;
    assert_eq!(restored.reason, RestoreReason::PageDidNotLoad);
    assert_eq!(restored.restored_version, "1.0.0");
    assert_eq!(folder_files(&update.pack())?, update.original);

    let update = Update::new()?;
    let transaction =
        replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
    update.apply(&transaction)?;
    transaction.observe("1.1.0", TrialEvent::BackendStartUnobservable)?;
    transaction.observe("1.1.0", TrialEvent::PageUnconfirmed)?;
    transaction.observe("1.1.0", TrialEvent::PageLoaded { nodes_registered: true })?;
    assert!(!exists(&update.transaction_root()), "a later page keeps the version");
    Ok(())
}

/// A start whose page never decides is pending at the next start, as the shared vectors
/// say, but three in a row restore the previous version, so a judgement never stays open.
#[test]
fn starts_whose_page_never_decides_restore_at_the_unwitnessed_limit() -> TestResult {
    for observed in
        [TrialEvent::BackendStartUnobservable, TrialEvent::BackendReady { nodes_registered: true }]
    {
        let update = Update::new()?;
        let transaction =
            replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
        update.apply(&transaction)?;
        let target = PackTree::at(&update.pack())?;
        transaction.observe("1.1.0", observed)?;
        for _ in 1..UNWITNESSED_ENDINGS_LIMIT {
            assert_eq!(transaction.settle(&target, TrialEnding::Unwitnessed)?, None);
            assert_eq!(ownership::read_identity(&target)?.version.to_string(), "1.1.0");
        }
        let restored =
            transaction.settle(&target, TrialEnding::Unwitnessed)?.ok_or("never restored")?;
        assert_eq!(restored.reason, RestoreReason::PageDidNotLoad, "{observed:?}");
        assert_eq!(folder_files(&update.pack())?, update.original);
    }
    Ok(())
}

#[test]
fn observations_of_another_version_or_after_the_verdict_change_nothing() -> TestResult {
    let update = Update::new()?;
    let transaction =
        replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
    update.apply(&transaction)?;
    transaction.observe("1.0.0", TrialEvent::BackendReady { nodes_registered: true })?;
    assert_eq!(transaction.pending()?.map(|pending| pending.phase), Some(JournalPhase::Applied));
    transaction.observe("1.1.0", TrialEvent::BackendReady { nodes_registered: true })?;
    transaction.observe("1.1.0", TrialEvent::PageLoaded { nodes_registered: true })?;
    assert!(!exists(&update.transaction_root()));
    transaction.observe("1.1.0", TrialEvent::PageFailed)?;
    let target = PackTree::at(&update.pack())?;
    assert_eq!(transaction.settle(&target, TrialEnding::BackendStopped)?, None);
    assert_eq!(ownership::read_identity(&target)?.version.to_string(), "1.1.0");
    assert!(exists(&update.pack().join("user-notes.txt")));
    Ok(())
}

#[test]
fn an_unsettled_transaction_refuses_another_install() -> TestResult {
    let update = Update::new()?;
    let transaction =
        replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?);
    update.apply(&transaction)?;
    let refused = update.apply(&transaction).err().ok_or("accepted a second install")?;
    assert_eq!(code(&refused), "busy");
    Ok(())
}

#[test]
fn a_fresh_install_is_owned_and_refuses_another_pack() -> TestResult {
    let root = tempfile::tempdir()?;
    let pack = install_registry_pack(root.path(), &pack_files("1.0.0"))?;
    let tracking = std::fs::read(pack.join(TRACKING_FILE_NAME))?;
    assert!(String::from_utf8(tracking.clone())?.contains("sugarcubes/core.py"));
    match PackFolder::at(&pack)?.ownership()? {
        Ownership::Owned(identity) => assert_eq!(identity.version.to_string(), "1.0.0"),
        other => return Err(format!("not owned: {other:?}").into()),
    }
    let recorded =
        ownership::read_digest_record(&PackTree::at(&pack)?, "1.0.0", &parse_tracking(&tracking)?)?;
    assert_eq!(recorded.map(|files| files.len()), Some(pack_files("1.0.0").len()));
    let mut other = pack_files("1.0.0");
    other.insert(
        "pyproject.toml".into(),
        "[project]\nname = \"Other\"\nversion = \"1.0.0\"\n".into(),
    );
    let archive = zip_bytes(&file_members(&other))?;
    let refused = write_registry_install(
        std::io::Cursor::new(archive),
        &root.path().join("other"),
        SUGARCUBES_PACK_ID,
    );
    assert_eq!(refused.err().as_ref().map(code), Some("pack-identity"));
    Ok(())
}

/// A pack with `count` extra files of about 16 KB, installed as `version`.
fn sized_pack(version: &str, count: usize) -> BTreeMap<String, String> {
    let mut files = pack_files(version);
    for index in 0..count {
        files.insert(
            format!("web/part{:02}/file{index:04}.js", index % 40),
            format!("// {version} {index}\n{}\n", "x".repeat(16_000)),
        );
    }
    files
}

/// Measures putting the previous version of a pack back, by phase. The timings are recorded,
/// never judged. Setting `SUGARSUBSTITUTE_RESTORE_TIMINGS` to a file keeps them and measures a
/// pack the size of SugarCubes (2,000 files); `SUGARSUBSTITUTE_RESTORE_ROOT` picks the drive.
/// Otherwise a 200-file pack keeps the suite quick.
#[test]
fn a_restore_records_its_phases_and_moves_every_file_back() -> TestResult {
    let root = match std::env::var_os("SUGARSUBSTITUTE_RESTORE_ROOT") {
        Some(base) => tempfile::tempdir_in(base)?,
        None => tempfile::tempdir()?,
    };
    let timings_file = std::env::var_os("SUGARSUBSTITUTE_RESTORE_TIMINGS");
    let count = if timings_file.is_some() { 2000 } else { 200 };
    let pack = install_registry_pack(root.path(), &sized_pack("1.0.0", count))?;
    let original = folder_files(&pack)?;
    let staged = PackTree::at(&root.path().join("staged"))?;
    let archive = zip_bytes(&file_members(&sized_pack("1.1.0", count)))?;
    let (extracted, extract) = archive::extract_registry_source_timed(
        std::io::Cursor::new(archive),
        &staged,
        ArchiveLimits::default(),
    )?;
    let new_entries = extracted.entries;
    let target = PackTree::at(&pack)?;
    let ownership::PackOwnership::Owned(owned) = ownership::inspect_pack_folder(&target)? else {
        return Err("fixture is not owned".into());
    };
    let transaction =
        replacement::TrackedReplacement::new(PackTree::at(&root.path().join("transaction"))?);
    let install = transaction.apply_timed(&replacement::ReplacementPlan {
        target: &target,
        source: &staged,
        old_entries: &owned.entries,
        new_entries: &new_entries,
        from_version: "1.0.0",
        to_version: "1.1.0",
    })?;
    let timings = transaction.roll_back_timed(&target)?.ok_or("nothing was restored")?;
    assert_eq!(folder_files(&pack)?, original);
    assert!(!exists(&root.path().join("transaction")));
    assert!(timings.moved >= count, "the backup sits beside the pack, so every file is moved");
    let record = serde_json::json!({
        "files": original.len(),
        "extractMs": extract.as_secs_f64() * 1000.0,
        "install": install.to_json(),
        "restore": timings.to_json(),
    });
    if let Some(path) = timings_file {
        std::fs::write(path, serde_json::to_vec_pretty(&record)?)?;
    }
    Ok(())
}
/// A restore cut off before any of its effects finishes the same restore when it runs again,
/// so files already moved back are never lost.
#[test]
fn a_restore_cut_off_at_any_point_finishes_when_repeated() -> TestResult {
    let clean = Update::new()?;
    let counted = replacement::TrackedReplacement::with_faults(
        PackTree::at(&clean.transaction_root())?,
        None,
        None,
    );
    clean.apply(&counted)?;
    let applied = counted.effects();
    counted.roll_back(&PackTree::at(&clean.pack())?)?;
    assert_eq!(folder_files(&clean.pack())?, clean.original);
    let restoring = counted.effects() - applied;
    assert!(restoring > 3);
    for cut in 1..=restoring {
        let update = Update::new()?;
        let transaction = replacement::TrackedReplacement::with_faults(
            PackTree::at(&update.transaction_root())?,
            Some(applied + cut),
            None,
        );
        update.apply(&transaction)?;
        let target = PackTree::at(&update.pack())?;
        assert!(transaction.roll_back(&target).is_err(), "effect {cut} did not fail");
        replacement::TrackedReplacement::new(PackTree::at(&update.transaction_root())?)
            .roll_back(&target)?;
        assert_eq!(folder_files(&update.pack())?, update.original, "effect {cut}");
        assert!(!exists(&update.transaction_root()), "effect {cut} left its transaction");
    }
    Ok(())
}

/// An install cut off before any of its effects, as a power cut would, leaves either the old
/// pack untouched or a journal the next start restores it from.
#[test]
fn an_install_cut_off_at_any_point_is_recovered_at_the_next_start() -> TestResult {
    let clean = Update::new()?;
    let counted = replacement::TrackedReplacement::with_faults(
        PackTree::at(&clean.transaction_root())?,
        None,
        None,
    );
    clean.apply(&counted)?;
    let installing = counted.effects();
    assert!(installing > 5);
    for cut in 1..=installing {
        let update = Update::new()?;
        let root = PackTree::at(&update.transaction_root())?;
        let transaction = replacement::TrackedReplacement::cut_before(root.clone(), cut);
        assert!(update.apply(&transaction).is_err(), "effect {cut} was not cut");
        let next = replacement::TrackedReplacement::new(root);
        next.settle(&PackTree::at(&update.pack())?, TrialEnding::Unwitnessed)?;
        assert_eq!(folder_files(&update.pack())?, update.original, "effect {cut}");
        assert_eq!(next.pending()?, None, "effect {cut} left its journal");
    }
    Ok(())
}
