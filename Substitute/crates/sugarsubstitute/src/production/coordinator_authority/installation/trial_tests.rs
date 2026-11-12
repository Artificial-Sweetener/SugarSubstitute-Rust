// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An unaccepted trial's verdict comes from observed facts, and acceptance never closes the app.
use super::*;
use sugarsubstitute_contracts::installation::{BadTrial, Undecided};

fn authority() -> Result<CoordinatorAuthority, Box<dyn Error>> {
    Ok(CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?)
}

#[test]
fn the_hosts_own_failure_is_bad_but_installation_storage_failures_are_not()
-> Result<(), Box<dyn Error>> {
    let mut storage = authority()?;
    storage.fail(Failure::Installation(AuthorityIssue::Adapter));
    assert!(storage.closing);
    assert_eq!(storage.installation.verdict(), TrialVerdict::NoVerdict(Undecided::CloseRequested));
    let mut host = authority()?;
    let Err(cause) = sugarsubstitute_contracts::execution::JobId::new(0) else {
        return Err("Zero job identity was accepted".into());
    };
    host.fail(Failure::Identity(cause));
    assert_eq!(host.installation.verdict(), TrialVerdict::Bad(BadTrial::StartupFailed));
    Ok(())
}

#[test]
fn the_first_ending_fact_decides_and_readiness_rules_out_a_bad_verdict()
-> Result<(), Box<dyn Error>> {
    let mut closed = authority()?;
    closed.observe_trial_close(CloseCause::Requested);
    closed.observe_trial_failure(FailedProcess::Host);
    assert_eq!(closed.installation.verdict(), TrialVerdict::NoVerdict(Undecided::CloseRequested));
    let mut session = authority()?;
    session.observe_trial_close(CloseCause::SessionEnd);
    session.fail(Failure::Installation(AuthorityIssue::Adapter));
    assert_eq!(session.installation.verdict(), TrialVerdict::NoVerdict(Undecided::SessionEnd));
    let mut backend = authority()?;
    backend.observe_trial_failure(FailedProcess::Backend);
    backend.observe_trial_close(CloseCause::Requested);
    assert_eq!(backend.installation.verdict(), TrialVerdict::NoVerdict(Undecided::BackendFailed));
    let mut ready = authority()?;
    ready.installation.observe_readiness();
    ready.observe_trial_failure(FailedProcess::Host);
    assert_eq!(
        ready.installation.verdict(),
        TrialVerdict::NoVerdict(Undecided::AcceptanceUnpublished)
    );
    Ok(())
}
