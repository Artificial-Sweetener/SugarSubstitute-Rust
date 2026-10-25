// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise native inherited file custody and installation identity through actual run adoption.
use super::*;
use crate::setup_storage::{InheritedSelection, LaunchInheritance, ReservedLaunch};
use std::fs::File;
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle};

#[allow(unsafe_code)]
fn child_copy(
    parent: &LaunchInheritance,
) -> Result<InheritedSelection, Box<dyn std::error::Error + Send + Sync>> {
    use windows::Win32::{
        Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE},
        System::Threading::GetCurrentProcess,
    };
    let mut duplicate = HANDLE::default();
    // SAFETY: the borrowed parent's file and process pseudo handles remain live; the unique
    // output is immediately adopted, then relinquished to the actual child capture routine.
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(parent.selection_handle().as_raw_handle()),
            GetCurrentProcess(),
            &mut duplicate,
            0,
            true,
            DUPLICATE_SAME_ACCESS,
        )
    }?;
    // SAFETY: successful duplication returned this unique file handle with no prior owner.
    let file = unsafe { File::from_raw_handle(duplicate.0) };
    let mut arguments = parent.arguments();
    arguments[1] = (file.into_raw_handle() as usize).to_string().into();
    // SAFETY: into_raw_handle relinquished the sole local owner. The handle cannot be reused
    // while this unowned live duplicate awaits capture, and no other party can close it.
    unsafe { crate::setup_storage::capture_launch_selection(arguments) }?
        .ok_or_else(|| "Child transfer absent".into())
}

#[test]
fn inherited_pin_survives_parent_drop_then_releases_after_coordinator_adoption() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 112 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
            return Err("Reservation absent".into());
        };
        let parent = reserved.prepare_inheritance(context)?;
        assert_eq!(parent.application().generation(), &generation);
        let child = child_copy(&parent)?;
        drop(parent);
        let next = before.prepare(
            Generation {
                id: GenerationId::new("2".repeat(32))?,
                package: PackageDigest::new("c".repeat(64))?,
            },
            PreparationIntent::Automatic,
        )?;
        assert!(store.compare_exchange(&before, &next, context).is_err());
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let InstallationLaunch::Application(application) =
            run.select_launch(Some(child), context)?
        else {
            return Err("Application absent".into());
        };
        store.compare_exchange(&before, &next, context)?;
        drop(run);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(application);
        assert!(InstallationRun::acquire(store, context).is_ok());
        Ok(())
    })
}

#[test]
fn foreign_inherited_record_cannot_authorize_a_matching_generation_in_another_installation()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 112 * 1024 * 1024, |context, _| {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        let directory = Directory::open(first.path(), false)?;
        let other = Directory::open(second.path(), false)?;
        accept_fixture(&directory, context)?;
        accept_fixture(&other, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let foreign = Arc::new(SelectionStore::open(other.path(), context)?);
        let before = foreign.load(context)?;
        let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
            return Err("Reservation absent".into());
        };
        let parent = reserved.prepare_inheritance(context)?;
        let child = child_copy(&parent)?;
        drop(parent);
        let mut run = InstallationRun::acquire(foreign.clone(), context)?;
        assert!(run.select_launch(Some(child), context).is_err());
        assert_eq!(foreign.load(context)?, before);
        let _released = std::fs::OpenOptions::new()
            .write(true)
            .open(directory.path().join(".selection-state/.substitute-selection.json"))?;
        assert!(matches!(run.select_launch(None, context)?, InstallationLaunch::Application(_)));
        Ok(())
    })
}
