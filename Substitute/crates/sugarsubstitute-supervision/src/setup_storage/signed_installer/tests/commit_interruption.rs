// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A crash at any step of the preparation commit hold (D21 step 3, D22) keeps the accepted
//! version starting, and the next preparation finishes the same update so it starts on the
//! following launch.
use super::*;
use crate::{
    application_launch::ApplicationLaunch,
    setup_storage::{
        UPDATE_RECEIPT_FILE, test_package::maintenance::accept_pending,
        test_worker::pipeline::Pipeline,
    },
};
use std::{path::Path, sync::Arc};
use sugarsubstitute_contracts::installation::Candidate;

/// Where in the commit hold the process ends.
#[derive(Clone, Copy, Debug)]
enum CrashAfter {
    /// The release history names the generation; no receipt yet.
    ReleaseHistory,
    /// The receipt is written; the selection record still says staging.
    Receipt,
    /// The selection record says prepared; the launcher transition has not run.
    PreparedRecord,
}

impl CrashAfter {
    /// Whether the files show the commit reached this point for a generation other than `old`.
    fn reached(self, installation: &Path, old: &str) -> bool {
        let releases = installation.join("releases");
        let created = |file: &str| {
            std::fs::read_dir(&releases).is_ok_and(|entries| {
                entries.flatten().any(|entry| {
                    entry.file_name().to_str().is_some_and(|name| name.len() == 32 && name != old)
                        && entry.path().join(file).is_file()
                })
            })
        };
        match self {
            Self::ReleaseHistory => created("application-update-v1.record"),
            Self::Receipt => created(UPDATE_RECEIPT_FILE),
            Self::PreparedRecord => {
                let selection = installation.join("state/selection/.substitute-selection.json");
                std::fs::read(selection)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .is_some_and(|value| {
                        value["candidate"]["phase"] == "prepared"
                            && value["candidate"]["generation"]["id"]
                                .as_str()
                                .is_some_and(|id| id != old)
                    })
            }
        }
    }
}

#[test]
fn a_crash_at_any_commit_step_keeps_the_accepted_version_and_the_update_starts_next_launch()
-> TestResult {
    for point in [CrashAfter::ReleaseHistory, CrashAfter::Receipt, CrashAfter::PreparedRecord] {
        let mut pipeline = Pipeline::new()?;
        let (fixture, before) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
            let fixture = Arc::new(Fixture::new(&context)?);
            let prepared = fixture
                .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
                .prepare(|| NOW, &context)?;
            let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
            drop(prepared.complete(&runtime, || NOW, &context)?);
            accept_pending(fixture.installation.path(), &context)?;
            let before = SelectionStore::open(fixture.installation.path(), &context)?
                .load(&context)?
                .ok_or("Selection absent")?;
            Ok((fixture, before))
        })?;
        drop(pipeline);
        let old = before.accepted().ok_or("Accepted absent")?.id.as_str().to_owned();
        let crashing = fixture.clone();
        let observed = old.clone();
        worker::run_reserved_expiring(
            SIGNED_INSTALLER_RESERVATION_BYTES,
            move |context, clock| {
                let installation = crashing.installation.path().to_owned();
                let watched = observed.clone();
                clock.observe_checks(move || point.reached(&installation, &watched))?;
                let prepared = crashing
                    .import(SignedPackage::new(&crashing.archive, 2)?, context)?
                    .prepare(|| NOW, context)?;
                let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
                assert!(prepared.complete(&runtime, || NOW, context).is_err(), "{point:?}");
                assert!(point.reached(crashing.installation.path(), &observed), "{point:?}");
                Ok(())
            },
        )?;
        worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, move |context, _| {
            let store = Arc::new(SelectionStore::open(fixture.installation.path(), context)?);
            let interrupted = store.load(context)?.ok_or("Selection absent")?;
            // The launch that follows the crash starts the accepted version.
            assert_eq!(interrupted.accepted(), before.accepted(), "{point:?}");
            assert!(interrupted.strikes().is_empty(), "{point:?}");
            drop(InstallationRun::acquire(store.clone(), context)?);
            let pending = interrupted.candidate().ok_or("Interrupted candidate lost")?.generation();
            // The next preparation finishes the same generation instead of starting over.
            let prepared = fixture
                .import(SignedPackage::new(&fixture.archive, 2)?, context)?
                .prepare(|| NOW, context)?;
            let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
            let candidate = Arc::new(prepared.complete(&runtime, || NOW, context)?);
            assert_eq!(
                candidate.directory(),
                fixture.installation.path().join("releases").join(pending.id.as_str()),
                "{point:?}"
            );
            let record = store.load(context)?.ok_or("Selection absent")?;
            assert!(
                matches!(record.candidate(), Some(Candidate::Prepared { generation }) if generation == pending),
                "{point:?}"
            );
            let trust = TrustStore::open(fixture.installation.path(), [92; 32], context)?;
            let receipt = trust.update_receipt(context)?.ok_or("Update receipt absent")?;
            assert_eq!(receipt.generation(), pending, "{point:?}");
            // And the launch after that starts it as a trial.
            let launch = ApplicationLaunch::trial(candidate);
            launch.prepare_trial(|| NOW, context)?;
            let trial = store.load(context)?.ok_or("Selection absent")?;
            assert!(matches!(trial.candidate(), Some(Candidate::Trial { .. })), "{point:?}");
            assert_eq!(trial.accepted(), before.accepted(), "{point:?}");
            fixture.preserved()
        })?;
    }
    Ok(())
}
