// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Slot exclusion ignores foreign readers, still excludes a second owner, and survives the
//! closure of every witness duplicate.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};

/// Live contention, never a storage failure, refused the acquisition.
fn refused(acquired: io::Result<SlotExclusion>) -> bool {
    matches!(acquired, Err(ref cause) if is_busy(cause))
}

#[test]
fn a_foreign_shared_reader_does_not_block_acquisition_and_a_second_owner_is_busy() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("run");
        with(context, |protection| {
            drop(protection.rendezvous(&path, context)?);
            // An indexer or antivirus scanner opens the file for reading with ordinary sharing.
            let foreign = std::fs::File::open(&path)?;
            let held = protection.exclusion(&path, context)?;
            assert!(refused(protection.exclusion(&path, context)));
            drop(foreign);
            assert!(protection.exclusion(&path, context).is_err());
            drop(held);
            drop(protection.exclusion(&path, context)?);
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn closing_witness_duplicates_never_releases_the_held_slot() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("run");
        with(context, |protection| {
            let held = protection.exclusion(&path, context)?;
            let witness = held.witness().try_clone()?;
            drop(held.witness().try_clone()?);
            assert!(refused(protection.exclusion(&path, context)));
            protection.verify_exclusive_rendezvous(&witness, &path, context)?;
            drop(held);
            assert!(protection.verify_exclusive_rendezvous(&witness, &path, context).is_err());
            drop(protection.exclusion(&path, context)?);
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn historical_whole_file_locks_and_slots_exclude_each_other() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("run");
        with(context, |protection| {
            let whole = protection.rendezvous(&path, context)?;
            whole.try_lock().map_err(io::Error::from)?;
            assert!(refused(protection.exclusion(&path, context)));
            drop(whole);
            let held = protection.exclusion(&path, context)?;
            let whole = protection.rendezvous(&path, context)?;
            assert!(whole.try_lock().is_err());
            drop(held);
            whole.try_lock().map_err(io::Error::from)?;
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn a_handoff_keeps_exclusion_when_every_sender_handle_closes() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("run");
        with(context, |protection| {
            let sender = protection.exclusion(&path, context)?;
            sender.hold_handoff(context, context.now())?;
            let receiver =
                protection.adopt_exclusion(sender.witness().try_clone()?, &path, context)?;
            drop(sender);
            assert!(refused(protection.exclusion(&path, context)));
            protection.verify_exclusive_rendezvous(receiver.witness(), &path, context)?;
            drop(receiver);
            drop(protection.exclusion(&path, context)?);
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn adoption_is_refused_without_a_live_sender_handoff() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("run");
        with(context, |protection| {
            let sender = protection.exclusion(&path, context)?;
            let witness = sender.witness().try_clone()?;
            // The sender holds its slot but never offered a handoff.
            assert!(protection.adopt_exclusion(witness.try_clone()?, &path, context).is_err());
            assert!(refused(protection.exclusion(&path, context)));
            // After the sender is gone, its witness no longer proves live exclusion.
            drop(sender);
            assert!(protection.adopt_exclusion(witness, &path, context).is_err());
            drop(protection.exclusion(&path, context)?);
            Ok(())
        })?;
        Ok(())
    })
}

#[test]
fn a_released_handoff_refuses_adoption_while_the_sender_keeps_its_slot() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("run");
        with(context, |protection| {
            let sender = protection.exclusion(&path, context)?;
            sender.hold_handoff(context, context.now())?;
            // Holding again is idempotent; a second sender in the same chain cannot also hold it.
            sender.hold_handoff(context, context.now())?;
            let receiver =
                protection.adopt_exclusion(sender.witness().try_clone()?, &path, context)?;
            let second = receiver.hold_handoff(context, context.now());
            assert!(matches!(second, Err(ref cause) if is_busy(cause)), "{second:?}");
            sender.release_handoff();
            receiver.hold_handoff(context, context.now())?;
            receiver.release_handoff();
            // Without a held handoff nobody can adopt, yet the sender's slot still excludes.
            assert!(
                protection.adopt_exclusion(sender.witness().try_clone()?, &path, context).is_err()
            );
            drop(receiver);
            assert!(refused(protection.exclusion(&path, context)));
            protection.verify_exclusive_rendezvous(sender.witness(), &path, context)?;
            drop(sender);
            drop(protection.exclusion(&path, context)?);
            Ok(())
        })?;
        Ok(())
    })
}
