// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize canonical matching and real finite source lifetime without native proof fabrication.
use super::*;
use state::Action;
use std::error::Error;
use sugarsubstitute_supervision::lifecycle_protocol::{
    FolderFragment, RawWindowsUnits, ReplyOutcome,
};
#[path = "tests/support.rs"]
mod support;
use support::*;
type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
#[test]
fn requested_two_fragment_progress_requires_exact_final_ack() -> TestResult {
    let identity = identity()?;
    let mut progress = Progress::new(identity, Moment::from_nanos(100), 32766)?;
    let first = pending(identity, 2)?;
    let reply = ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: 16384 };
    assert_eq!(progress.reply(first, first.header, reply, None)?, Action::Advance(16384));
    assert_eq!(progress.offset, 0);
    progress.apply(Action::Advance(16384));
    let second = pending(identity, 3)?;
    assert!(progress.reply(second, first.header, reply, None).is_err());
    assert!(
        progress
            .reply(
                second,
                second.header,
                ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: 16384 },
                None
            )
            .is_err()
    );
    let action = progress.reply(
        second,
        second.header,
        ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: 32766 },
        None,
    )?;
    assert_eq!(action, Action::PrepareMain);
    progress.apply(action);
    assert_eq!(progress.phase, Phase::EnvironmentWaiting);
    assert_eq!(progress.original, Moment::from_nanos(100));
    Ok(())
}
#[test]
fn environment_final_chunk_requires_authorization_not_assembly_ack() -> TestResult {
    let identity = identity()?;
    let mut progress = Progress::new(identity, Moment::from_nanos(100), 32766)?;
    progress.phase = Phase::Environment;
    let first = pending(identity, 2)?;
    assert!(
        progress
            .reply(first, first.header, ReplyOutcome::EnvironmentAuthorized(identity), None)
            .is_err()
    );
    progress.apply(progress.reply(
        first,
        first.header,
        ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: 16384 },
        None,
    )?);
    let final_chunk = pending(identity, 3)?;
    assert!(
        progress
            .reply(
                final_chunk,
                final_chunk.header,
                ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: 32766 },
                None
            )
            .is_err()
    );
    assert_eq!(
        progress.reply(
            final_chunk,
            final_chunk.header,
            ReplyOutcome::EnvironmentAuthorized(identity),
            None
        )?,
        Action::AuthorizeEnvironment
    );
    Ok(())
}
#[test]
fn exact_pid_identity_and_admission_are_required_before_browser_authorization() -> TestResult {
    let identity = identity()?;
    let mut progress = Progress::new(identity, Moment::from_nanos(100), 1)?;
    progress.phase = Phase::Browser;
    let pending = pending(identity, 2)?;
    let pid = NonZeroU32::new(73).ok_or("Zero PID")?;
    let reply = ReplyOutcome::BrowserAuthorized { identity, browser_process_id: pid };
    assert!(
        progress
            .reply(Pending { admitted: false, ..pending }, pending.header, reply, Some(pid))
            .is_err()
    );
    assert!(progress.reply(pending, pending.header, reply, NonZeroU32::new(74)).is_err());
    let mut foreign = identity;
    foreign.navigation = sugarsubstitute_contracts::execution::Generation::new(2)?;
    assert!(
        progress
            .reply(
                pending,
                pending.header,
                ReplyOutcome::BrowserAuthorized { identity: foreign, browser_process_id: pid },
                Some(pid)
            )
            .is_err()
    );
    assert_eq!(
        progress.reply(pending, pending.header, reply, Some(pid))?,
        Action::AuthorizeBrowser
    );
    progress.phase = Phase::Closed;
    assert!(progress.reply(pending, pending.header, reply, Some(pid)).is_err());
    Ok(())
}
#[test]
fn exact_raw_windows_fragments_preserve_unpaired_surrogates_and_maximum_partition() -> TestResult {
    let identity = identity()?;
    let mut units = vec![0xd800; 32766];
    units[16383] = 0xdcff;
    let first =
        FolderFragment::new(identity, 32766, 0, RawWindowsUnits::from_units(&units[..16384])?)?;
    let second =
        FolderFragment::new(identity, 32766, 16384, RawWindowsUnits::from_units(&units[16384..])?)?;
    assert!(first.units().units().chain(second.units().units()).eq(units.iter().copied()));
    assert!(FolderFragment::new(identity, 32766, 1, first.units()).is_err());
    Ok(())
}
#[test]
fn actual_request_folder_and_context_retain_original_reservation_after_worker_join() -> TestResult {
    let rig = Rig::new()?;
    let transaction = transaction(&rig)?;
    assert_eq!(transaction.requested.units, r"E:\profile".encode_utf16().collect::<Vec<_>>());
    assert_eq!(transaction.original_deadline(), Moment::from_nanos(100));
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, MainWindowRequest::KNOWN_STORAGE_BYTES);
    drop(transaction);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn close_rejects_actual_observation_without_losing_its_original_facts() -> TestResult {
    let rig = Rig::new()?;
    let mut transaction = transaction(&rig)?;
    let claim = transaction
        .context
        .memory()
        .claim(sugarsubstitute_desktop::MainEnvironmentFacts::MAX_STORAGE_BYTES)?;
    let facts = MainEnvironmentFacts::new(vec![69, 58, 92, 0xd800], claim)?;
    transaction.close();
    let mut original =
        Some(sugarsubstitute_desktop::DesktopStartupObservation::EnvironmentPrepared {
            identity: transaction.identity(),
            facts,
        });
    assert!(matches!(transaction.observe(&mut original), Err(ProfileError::Closed)));
    drop(transaction);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, MainWindowRequest::KNOWN_STORAGE_BYTES);
    let sugarsubstitute_desktop::DesktopStartupObservation::EnvironmentPrepared { facts, .. } =
        original.ok_or("Original observation missing")?
    else {
        return Err("Observation replaced".into());
    };
    assert_eq!(facts.as_utf16(), &[69, 58, 92, 0xd800]);
    drop(facts);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn actual_scope_cancellation_after_completion_vetoes_future_profile_admission() -> TestResult {
    let rig = Rig::new()?;
    let transaction = transaction(&rig)?;
    assert!(transaction.check().is_ok());
    rig.scope.cancel();
    assert!(matches!(transaction.check(), Err(ProfileError::Cancelled)));
    drop(transaction);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_original_cutoff_is_not_renewed_after_constructor_completion() -> TestResult {
    let rig = Rig::new()?;
    let transaction = transaction(&rig)?;
    assert!(transaction.check().is_ok());
    rig.expire();
    assert!(matches!(transaction.check(), Err(ProfileError::Deadline)));
    assert_eq!(transaction.original_deadline(), Moment::from_nanos(100));
    Ok(())
}

#[test]
fn observed_environment_and_controller_keep_exact_stage_and_original_custody() -> TestResult {
    let rig = Rig::new()?;
    let mut transaction = transaction(&rig)?;
    let claim = transaction.context.memory().claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)?;
    let facts = MainEnvironmentFacts::new(vec![69, 58, 92, 0xd800], claim)?;
    let mut environment =
        Some(sugarsubstitute_desktop::DesktopStartupObservation::EnvironmentPrepared {
            identity: transaction.identity(),
            facts,
        });
    assert!(matches!(transaction.observe(&mut environment), Err(ProfileError::Order)));
    assert!(environment.is_some());
    transaction.progress.apply(Action::PrepareMain);
    assert!(transaction.observe(&mut environment)?);
    assert!(environment.is_none());
    assert_eq!(
        transaction.environment.as_ref().ok_or("Environment lost")?.as_utf16(),
        &[69, 58, 92, 0xd800]
    );
    let mut controller =
        Some(sugarsubstitute_desktop::DesktopStartupObservation::ControllerPrepared {
            identity: transaction.identity(),
            browser_process_id: 73,
        });
    assert!(matches!(transaction.observe(&mut controller), Err(ProfileError::Order)));
    assert!(controller.is_some());
    transaction.progress.apply(Action::AuthorizeEnvironment);
    assert!(transaction.observe(&mut controller)?);
    assert!(controller.is_none());
    assert_eq!(transaction.browser, NonZeroU32::new(73));
    assert!(!transaction.authorized());
    assert!(!transaction.awaiting_reply());
    drop(transaction);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn completed_transaction_stays_idle_after_its_original_startup_cutoff() -> TestResult {
    let rig = Rig::new()?;
    let mut transaction = transaction(&rig)?;
    transaction.progress.apply(Action::AuthorizeBrowser);
    assert!(transaction.authorized());
    rig.expire();
    assert!(!transaction.activity_required()?);
    assert!(!transaction.awaiting_reply());
    assert_eq!(transaction.original_deadline(), Moment::from_nanos(100));
    transaction.close();
    assert!(matches!(transaction.activity_required(), Err(ProfileError::Closed)));
    Ok(())
}

#[test]
fn routing_matches_only_the_complete_original_admitted_header_without_consuming_it() -> TestResult {
    let rig = Rig::new()?;
    let mut transaction = transaction(&rig)?;
    let p = pending(transaction.identity(), 2)?;
    assert!(!transaction.expects_reply(p.header));
    transaction.pending = Some(Pending { admitted: false, ..p });
    assert!(!transaction.expects_reply(p.header));
    transaction.pending = Some(p);
    assert!(transaction.expects_reply(p.header));
    let mut other = p.header;
    other.deadline_qpc += 1;
    assert!(!transaction.expects_reply(other));
    other = p.header;
    other.request = Some(sugarsubstitute_contracts::lifecycle::RequestId::new(3)?);
    assert!(!transaction.expects_reply(other));
    other = p.header;
    other.connection = Some(sugarsubstitute_contracts::lifecycle::ConnectionId::new(2)?);
    assert!(!transaction.expects_reply(other));
    other = p.header;
    other.run.generation = sugarsubstitute_contracts::lifecycle::RunGeneration::new(2)?;
    assert!(!transaction.expects_reply(other));
    assert_eq!(transaction.pending, Some(p));
    transaction.close();
    assert!(!transaction.expects_reply(p.header));
    Ok(())
}

#[test]
fn diagnosis_preserves_bounded_coordinator_and_native_stage_classifications() -> TestResult {
    use sugarsubstitute_contracts::lifecycle::{LifecycleFailureCode, LifecycleRejection};
    use sugarsubstitute_supervision::lifecycle_protocol::{
        MainFailureCode, MainStage, MainStageFailure,
    };
    assert_eq!(
        ProfileError::CoordinatorFailed(LifecycleFailureCode::Cleanup).to_string(),
        "Coordinator failed: Cleanup"
    );
    assert_eq!(
        ProfileError::CoordinatorRejected(LifecycleRejection::ProfileContention).to_string(),
        "Coordinator rejected: ProfileContention"
    );
    let failure = MainStageFailure {
        identity: identity()?,
        stage: MainStage::Browser,
        failure: MainFailureCode::OutsideOwnedFamily,
    };
    assert_eq!(
        ProfileError::Stage(failure).to_string(),
        "Native main stage failed: Browser (OutsideOwnedFamily)"
    );
    Ok(())
}
