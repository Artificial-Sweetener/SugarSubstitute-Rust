// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real trial admission joins current signed authority to exact native installation custody.
use super::*;
use crate::setup_storage::{InstallationRun, SelectionStore};
use sugarsubstitute_contracts::installation::Candidate;
#[allow(unsafe_code)]
mod transfer;

#[test]
fn admitted_trial_retains_payloads_and_run_custody_without_claiming_health() -> TestResult {
    run(1, |case| {
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let generation = candidate.generation().clone();
        let store = Arc::new(SelectionStore::open(&case.directory.path, case.context)?);
        let mut owner = InstallationRun::acquire(store.clone(), case.context)?;
        let admitted = candidate.begin_trial(
            &mut owner,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_eq!(&admitted.trial().attempt().generation, &generation);
        let state = store.load(case.context)?.ok_or("Trial missing")?;
        assert!(
            matches!(state.candidate(), Some(Candidate::Trial { attempt }) if attempt == admitted.trial().attempt())
        );
        assert!(state.accepted().is_none());
        drop(owner);
        assert!(InstallationRun::acquire(store.clone(), case.context).is_err());
        assert!(
            std::fs::write(admitted.directory().join("sugarsubstitute.exe"), b"changed").is_err()
        );
        case.preserved()?;
        drop(admitted);
        let _next = InstallationRun::acquire(store, case.context)?;
        case.preserved()
    })
}

#[test]
fn expiry_and_withdrawal_after_preparation_never_start_a_trial() -> TestResult {
    run(1, |case| {
        let store = Arc::new(SelectionStore::open(&case.directory.path, case.context)?);
        let mut owner = InstallationRun::acquire(store.clone(), case.context)?;
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let before = store.load(case.context)?;
        assert!(
            candidate
                .begin_trial(
                    &mut owner,
                    &case.signed.authorization(case.trust),
                    || EXPIRES,
                    case.context
                )
                .is_err()
        );
        assert_eq!(store.load(case.context)?, before);
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let withdrawn = SignedPackage::new(b"replacement package", 2)?;
        assert!(
            candidate
                .begin_trial(&mut owner, &withdrawn.authorization(case.trust), || NOW, case.context)
                .is_err()
        );
        assert_eq!(store.load(case.context)?, before);
        case.preserved()
    })
}

#[test]
fn same_logical_installation_id_does_not_authorize_a_foreign_native_run() -> TestResult {
    run(1, |case| {
        let other = tempfile::tempdir()?;
        let directory = Directory::open(other.path(), false)?;
        fixture::installed(&directory, case.context)?;
        let _trust = case.signed.initialize(&directory.path, case.context)?;
        let foreign = Arc::new(SelectionStore::open(&directory.path, case.context)?);
        let before = foreign.load(case.context)?;
        let mut owner = InstallationRun::acquire(foreign.clone(), case.context)?;
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert!(
            candidate
                .begin_trial(
                    &mut owner,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        assert_eq!(foreign.load(case.context)?, before);
        let state = SelectionStore::open(&case.directory.path, case.context)?
            .load(case.context)?
            .ok_or("Candidate absent")?;
        assert!(matches!(state.candidate(), Some(Candidate::Prepared { .. })));
        case.preserved()
    })
}

#[test]
fn candidate_cancelled_after_preparation_cannot_admit_a_stale_trial() -> TestResult {
    run(1, |case| {
        let candidate = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        let store = Arc::new(SelectionStore::open(&case.directory.path, case.context)?);
        let before = store.load(case.context)?.ok_or("Candidate absent")?;
        let cancelled = before.cancel_preparation(candidate.generation())?;
        store.compare_exchange(&before, &cancelled, case.context)?;
        let mut owner = InstallationRun::acquire(store.clone(), case.context)?;
        assert!(
            candidate
                .begin_trial(
                    &mut owner,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        assert_eq!(store.load(case.context)?, Some(cancelled));
        case.preserved()
    })
}
