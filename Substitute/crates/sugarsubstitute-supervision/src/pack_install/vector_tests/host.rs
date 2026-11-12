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
