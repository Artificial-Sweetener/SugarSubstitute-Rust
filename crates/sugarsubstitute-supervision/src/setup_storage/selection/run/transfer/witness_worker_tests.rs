// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify authenticated duplicate receipt and recovery on separately admitted storage workers.
use super::*;
use crate::setup_storage::{
    SIGNED_INSTALLER_RESERVATION_BYTES,
    test_worker::{self as worker, TestResult},
};
use std::{os::windows::io::OwnedHandle, sync::Mutex};

type Failure = Box<dyn std::error::Error + Send + Sync>;
type Transfer = (tempfile::TempDir, PathBuf, [OwnedHandle; 2]);

/// Retire all creator objects while genuine independently owned destination handles remain live.
fn transferred() -> Result<Transfer, Failure> {
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
        let root = tempfile::tempdir()?;
        let application = shell_tests::trial(root.path(), context)?;
        let generation = application.directory().to_owned();
        let transfer = application.prepare_witness(context)?;
        let handles = [
            transfer.handles()[0].try_clone_to_owned()?,
            transfer.handles()[1].try_clone_to_owned()?,
        ];
        for handle in &handles {
            let mut flags = 0;
            // SAFETY: these exact live duplicates are owned above throughout the native query.
            unsafe {
                windows::Win32::Foundation::GetHandleInformation(
                    windows::Win32::Foundation::HANDLE(handle.as_raw_handle()),
                    &mut flags,
                )
            }?;
            assert_eq!(flags & windows::Win32::Foundation::HANDLE_FLAG_INHERIT.0, 0);
        }
        drop(application);
        drop(transfer);
        *output.lock().map_err(|_| "Transfer retention poisoned")? =
            Some((root, generation, handles));
        Ok(())
    })?;
    let value = retained.lock().map_err(|_| "Transfer retention poisoned")?.take();
    value.ok_or_else(|| "Actual transfer outputs were lost".into())
}

#[test]
fn authenticated_noninheritable_outputs_fit_the_independent_storage_reservation() -> TestResult {
    let (root, generation, handles) = transferred()?;
    worker::run_reserved(TRIAL_WITNESS_RESERVATION_BYTES, move |context, _| {
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        let original = store.load(context)?.ok_or("Selection absent")?;
        let witness =
            TrialWitness::receive_transferred(store.clone(), handles, &generation, context)?;
        assert_eq!(store.load(context)?, Some(original.clone()));
        let rejected = witness.finish_settled(context)?;
        assert_eq!(rejected, original.reject(witness.attempt())?);
        assert_eq!(witness.finish_settled(context)?, rejected);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(witness);
        let _successor = InstallationRun::acquire(store, context)?;
        Ok(())
    })
}

#[test]
fn admission_job_expiry_does_not_destroy_transferred_witness_custody() -> TestResult {
    let (root, generation, handles) = transferred()?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    worker::run_reserved_expiring(TRIAL_WITNESS_RESERVATION_BYTES, move |context, clock| {
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let original = store.load(context)?.ok_or("Selection absent")?;
        let witness =
            TrialWitness::receive_transferred(store.clone(), handles, &generation, context)?;
        clock.expire();
        match witness.finish_settled(context) {
            Err(SelectionFailure::Storage(cause)) => {
                assert_eq!(cause.kind(), io::ErrorKind::TimedOut)
            }
            _ => return Err("Expired storage context authorized trial finalization".into()),
        }
        *output.lock().map_err(|_| "Witness retention poisoned")? =
            Some((root, store, original, witness));
        Ok(())
    })?;
    worker::run_reserved(TRIAL_WITNESS_RESERVATION_BYTES, move |context, _| {
        let retained = retained.lock().map_err(|_| "Witness retention poisoned")?;
        let (_root, store, original, witness) =
            retained.as_ref().ok_or("Witness custody absent")?;
        assert_eq!(store.load(context)?.as_ref(), Some(original));
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        let rejected = witness.finish_settled(context)?;
        assert_eq!(rejected, original.reject(witness.attempt())?);
        assert_eq!(store.load(context)?, Some(rejected));
        Ok(())
    })
}
