// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual evidence owner with typed facts and no native resources.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, NamespaceId, ProcessId,
};

fn process(id: u64) -> Result<ProcessKey> {
    Ok(ProcessKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ProcessId::new(id)?,
    })
}
fn bound(expected: u32) -> Result<Evidence> {
    let mut evidence = Evidence::new(0, expected)?;
    evidence.bind(process(1)?, JobId::new(1)?)?;
    Ok(evidence)
}
fn ready(expected: u32) -> Result<Evidence> {
    let mut evidence = bound(expected)?;
    evidence.protocol(FixtureObservation::Prepared)?;
    evidence.protocol(FixtureObservation::Ready)?;
    Ok(evidence)
}
fn cancelled(expected: u32) -> Result<Evidence> {
    let mut evidence = ready(expected)?;
    evidence.request_cancel(JobId::new(1)?)?;
    evidence.observe(RegistryObservation::RootObservationRetired {
        process: process(1)?,
        job: JobId::new(1)?,
        outcome: RootObservationOutcome::Cancelled,
    })?;
    Ok(evidence)
}
fn responded(expected: u32) -> Result<Evidence> {
    let mut evidence = cancelled(expected)?;
    evidence.request_challenge()?;
    evidence.protocol(FixtureObservation::Responded)?;
    Ok(evidence)
}
fn report(correlation: NativeCorrelation) -> FixtureSessionReport {
    FixtureSessionReport {
        correlation,
        ready: true,
        challenge_sent: true,
        responded: true,
        release_sent: true,
    }
}

#[test]
fn correlations_are_positive_checked_and_binding_cannot_replace_current_identity() -> Result<()> {
    assert_eq!(Evidence::new(0, 259)?.correlation.get(), 1);
    assert_eq!(Evidence::new(u64::MAX - 1, 37)?.correlation.get(), u64::MAX);
    assert!(Evidence::new(u64::MAX, 0).is_err());
    let mut evidence = bound(259)?;
    assert!(evidence.bind(process(2)?, JobId::new(2)?).is_err());
    assert_eq!(evidence.process, Some(process(1)?));
    assert_eq!(evidence.root_job, Some(JobId::new(1)?));
    Ok(())
}

#[test]
fn prepared_ready_and_response_require_actual_ordered_predecessors() -> Result<()> {
    let mut evidence = Evidence::new(0, 259)?;
    assert!(evidence.protocol(FixtureObservation::Ready).is_err());
    evidence.protocol(FixtureObservation::Prepared)?;
    assert!(evidence.protocol(FixtureObservation::Prepared).is_err());
    assert!(evidence.protocol(FixtureObservation::Ready).is_err());
    evidence.bind(process(1)?, JobId::new(1)?)?;
    evidence.protocol(FixtureObservation::Ready)?;
    assert!(evidence.protocol(FixtureObservation::Ready).is_err());
    assert!(evidence.protocol(FixtureObservation::Responded).is_err());
    assert!(!evidence.responded);
    assert!(evidence.request_challenge().is_err());
    assert!(!evidence.challenge_requested);
    Ok(())
}

#[test]
fn requested_cancel_does_not_prove_retirement_or_admit_challenge() -> Result<()> {
    let mut evidence = ready(259)?;
    assert!(evidence.request_cancel(JobId::new(2)?).is_err());
    assert!(evidence.cancel_requested.is_none());
    evidence.request_cancel(JobId::new(1)?)?;
    assert!(evidence.request_cancel(JobId::new(1)?).is_err());
    assert!(evidence.request_challenge().is_err());
    assert!(evidence.protocol(FixtureObservation::Responded).is_err());
    assert!(evidence.retirement.is_none());
    assert!(!evidence.complete());
    Ok(())
}

#[test]
fn foreign_mismatched_timeout_or_unrequested_retirement_never_becomes_cancelled_proof() -> Result<()>
{
    for (foreign, requested, job, outcome) in [
        (true, true, 1, RootObservationOutcome::Cancelled),
        (false, true, 2, RootObservationOutcome::Cancelled),
        (false, true, 1, RootObservationOutcome::TimedOut),
        (false, false, 1, RootObservationOutcome::Cancelled),
    ] {
        let mut evidence = ready(259)?;
        if requested {
            evidence.request_cancel(JobId::new(1)?)?;
        }
        assert!(
            evidence
                .observe(RegistryObservation::RootObservationRetired {
                    process: process(if foreign { 2 } else { 1 })?,
                    job: JobId::new(job)?,
                    outcome
                })
                .is_err()
        );
        assert!(evidence.retirement.is_none());
        assert!(evidence.request_challenge().is_err());
    }
    let mut evidence = cancelled(259)?;
    assert!(
        evidence
            .observe(RegistryObservation::RootObservationRetired {
                process: process(1)?,
                job: JobId::new(1)?,
                outcome: RootObservationOutcome::Cancelled
            })
            .is_err()
    );
    Ok(())
}

#[test]
fn expected_failure_cannot_mask_native_cleanup_omission_wrong_stage_or_owner_panic() {
    assert!(expected_failure("process_observation_not_exit", 0, 0, false));
    for (stage, count, omitted, panic) in [
        ("process_observation", 0, 0, false),
        ("process_observation_not_exit", 1, 0, false),
        ("process_observation_not_exit", 0, 1, false),
        ("process_observation_not_exit", 0, 0, true),
        ("process_observation_not_exit", 8, u64::MAX, true),
    ] {
        assert!(!expected_failure(stage, count, omitted, panic));
    }
}

#[test]
fn expected_failure_requires_actual_cancelled_retirement_and_accepts_only_once() -> Result<()> {
    let mut evidence = ready(259)?;
    assert!(!evidence.accept_expected_failure("process_observation_not_exit", 0, 0, false));
    evidence.request_cancel(JobId::new(1)?)?;
    assert!(!evidence.accept_expected_failure("process_observation_not_exit", 0, 0, false));
    assert!(!evidence.expected_failure);
    evidence.observe(RegistryObservation::RootObservationRetired {
        process: process(1)?,
        job: JobId::new(1)?,
        outcome: RootObservationOutcome::Cancelled,
    })?;
    for (cleanup, omitted, panic) in [(1, 0, false), (0, 1, false), (0, 0, true)] {
        assert!(!evidence.accept_expected_failure(
            "process_observation_not_exit",
            cleanup,
            omitted,
            panic
        ));
        assert!(!evidence.expected_failure);
    }
    assert!(evidence.accept_expected_failure("process_observation_not_exit", 0, 0, false));
    assert!(evidence.expected_failure);
    assert!(!evidence.accept_expected_failure("process_observation_not_exit", 0, 0, false));
    Ok(())
}

#[test]
fn release_is_not_exit_and_early_or_wrong_actual_code_is_retained_on_error() -> Result<()> {
    let mut evidence = ready(259)?;
    evidence.request_release()?;
    assert!(evidence.request_release().is_err());
    assert!(evidence.request_challenge().is_err());
    assert!(evidence.exited.is_none());
    assert!(!evidence.complete());
    for expected in [259, 37] {
        let mut evidence = ready(expected)?;
        assert!(
            evidence
                .observe(RegistryObservation::RootExited { process: process(1)?, code: 259 })
                .is_err()
        );
        assert_eq!(evidence.exited, Some(259));
        assert!(
            evidence
                .observe(RegistryObservation::RootExited { process: process(1)?, code: 37 })
                .is_err()
        );
        assert_eq!(evidence.exited, Some(259));
    }
    let mut evidence = responded(259)?;
    evidence.request_release()?;
    assert!(
        evidence
            .observe(RegistryObservation::RootExited { process: process(2)?, code: 259 })
            .is_err()
    );
    assert!(evidence.exited.is_none());
    evidence.observe(RegistryObservation::RootExited { process: process(1)?, code: 259 })?;
    assert_eq!(evidence.exited, Some(259));
    assert!(!evidence.settled);
    Ok(())
}

#[test]
fn response_requires_challenge_and_cannot_duplicate_an_actual_response() -> Result<()> {
    let mut evidence = cancelled(259)?;
    assert!(evidence.protocol(FixtureObservation::Responded).is_err());
    evidence.request_challenge()?;
    assert!(evidence.request_challenge().is_err());
    evidence.protocol(FixtureObservation::Responded)?;
    assert!(evidence.protocol(FixtureObservation::Responded).is_err());
    assert!(evidence.responded);
    Ok(())
}

#[test]
fn incomplete_or_foreign_report_cannot_supply_missing_protocol_facts() -> Result<()> {
    for field in 0..4 {
        let mut evidence = responded(259)?;
        evidence.request_release()?;
        let mut terminal = report(evidence.correlation);
        match field {
            0 => terminal.ready = false,
            1 => terminal.challenge_sent = false,
            2 => terminal.responded = false,
            _ => terminal.release_sent = false,
        }
        assert!(evidence.report(terminal).is_err());
        assert_eq!(evidence.session_report, Some(terminal));
        assert!(!evidence.complete());
    }
    let mut evidence = ready(259)?;
    assert!(evidence.report(report(NativeCorrelation::new(2)?)).is_err());
    assert!(evidence.session_report.is_none());
    assert!(evidence.report(report(evidence.correlation)).is_err());
    assert!(!evidence.responded);
    assert!(!evidence.challenge_requested);
    assert!(evidence.session_report.is_some());
    assert!(!evidence.complete());
    Ok(())
}

#[test]
fn settlement_requires_original_shutdown_and_cannot_replace_actual_exit() -> Result<()> {
    let mut evidence = ready(259)?;
    assert!(
        evidence
            .observe(RegistryObservation::FamilySettled { process: process(1)?, shutdown: None })
            .is_err()
    );
    evidence.begin_stop(ShutdownId::new(1)?)?;
    assert!(evidence.begin_stop(ShutdownId::new(2)?).is_err());
    for (id, shutdown) in
        [(2, Some(ShutdownId::new(1)?)), (1, Some(ShutdownId::new(2)?)), (1, None)]
    {
        assert!(
            evidence
                .observe(RegistryObservation::FamilySettled { process: process(id)?, shutdown })
                .is_err()
        );
        assert!(!evidence.settled);
    }
    evidence.observe(RegistryObservation::FamilySettled {
        process: process(1)?,
        shutdown: Some(ShutdownId::new(1)?),
    })?;
    assert!(evidence.settled);
    assert!(evidence.exited.is_none());
    assert!(!evidence.complete());
    Ok(())
}

#[test]
fn complete_valid_sequence_requires_each_actual_milestone_and_explicit_retirement() -> Result<()> {
    let mut evidence = responded(259)?;
    evidence.request_release()?;
    evidence.report(report(evidence.correlation))?;
    assert!(evidence.report(report(evidence.correlation)).is_err());
    evidence.observe(RegistryObservation::RootExited { process: process(1)?, code: 259 })?;
    evidence.begin_stop(ShutdownId::new(1)?)?;
    evidence.observe(RegistryObservation::FamilySettled {
        process: process(1)?,
        shutdown: Some(ShutdownId::new(1)?),
    })?;
    assert!(!evidence.complete());
    // The fake error boundary supplies the same public facts as a registry error;
    // the parent separately sets retired only after actual registry retirement.
    assert!(evidence.accept_expected_failure("process_observation_not_exit", 0, 0, false));
    assert!(!evidence.complete());
    evidence.retired = true;
    assert!(evidence.complete());
    assert!(
        evidence.observe(RegistryObservation::ContainedStarted { process: process(1)? }).is_err()
    );
    Ok(())
}
