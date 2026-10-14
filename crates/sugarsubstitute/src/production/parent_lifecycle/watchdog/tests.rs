// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only native queue/issue facts and timestamps; exercise the real Parent/watchdog owners.
//! No peer, native admission, UI progress or retirement authority is manufactured by these tests.
use super::*;
use sugarsubstitute_application::supervision::{UiChallengePhase, UiWatchdog};
use sugarsubstitute_contracts::lifecycle::{ConnectionId, RequestId, UiChallengeId};
type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn moment(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}
fn actor() -> TestResult<ParentLifecycle> {
    use sugarsubstitute_contracts::lifecycle::{
        CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
    };
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )?;
    let mut parent = ParentLifecycle::new(
        run,
        sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        sugarsubstitute_contracts::configuration::ProfileId::new("Watchdog_1".into())?,
        ConnectionId::new(1)?,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        storage,
    )?;
    parent.watchdog = Some(UiWatchdog::new(run, moment(0)));
    parent.ui_activity(UiWatchdogActivity::VisibleActive, moment(0))?;
    Ok(parent)
}
fn queue(parent: &mut ParentLifecycle, seconds: u64, operation: u64) -> TestResult<Challenge> {
    let now = moment(seconds);
    let effect = parent.watchdog.as_mut().ok_or("Watchdog owner missing")?.poll(now)?;
    let Some(UiWatchdogEffect::Challenge { challenge: key }) = effect else {
        return Err("Original challenge effect missing".into());
    };
    let until = now.checked_add(UI_SUSPICION_THRESHOLD)?;
    parent.challenge = Some(Challenge {
        key,
        header: WireHeader {
            run: parent.run,
            connection: Some(parent.connection),
            request: None,
            deadline_qpc: i64::try_from(until.as_nanos())?,
        },
        queued_at: now,
        until,
        operation: None,
        issued_at: None,
    });
    parent.queue_challenge_write(key, operation)?;
    parent.challenge.ok_or_else(|| "Original queued record missing".into())
}
fn issue(parent: &mut ParentLifecycle, challenge: Challenge, at: u64, now: u64) -> TestResult {
    let operation = challenge.operation.ok_or("Original queue operation missing")?;
    // This is the external native-issue fake; the actual same WriteReceipt validates ordering.
    let mut receipt = super::super::write_receipt::WriteReceipt::new(operation, 64);
    receipt.issue(operation)?;
    parent.challenge_issued(challenge.key, operation, moment(at), moment(now))?;
    Ok(())
}
fn response(challenge: Challenge) -> TestResult<WireHeader> {
    Ok(WireHeader { request: Some(RequestId::new(2)?), ..challenge.header })
}
#[test]
fn issued_response_after_activity_retirement_consumes_original_without_progress_or_admission_close()
-> TestResult {
    let mut parent = actor()?;
    let original = queue(&mut parent, 5, 1)?;
    issue(&mut parent, original, 5, 5)?;
    parent.ui_activity(UiWatchdogActivity::Inactive, moment(6))?;
    let before = parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot();
    assert!(parent.challenge.is_none());
    assert_eq!(before.pending, None);
    parent.consume_ui_response(response(original)?, original.key.id, moment(7))?;
    assert_eq!(parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot(), before);
    assert!(!parent.closed && parent.failure().is_none());
    assert!(parent.retired_challenges.iter().all(Option::is_none));
    assert!(parent.consume_ui_response(response(original)?, original.key.id, moment(8)).is_err());
    Ok(())
}
#[test]
fn unissued_and_actually_revoked_challenge_cannot_acknowledge_ui_processing() -> TestResult {
    let mut parent = actor()?;
    let original = queue(&mut parent, 5, 1)?;
    parent.ui_activity(UiWatchdogActivity::Inactive, moment(6))?;
    assert!(parent.consume_ui_response(response(original)?, original.key.id, moment(7)).is_err());
    assert!(parent.challenge_issued(original.key, 2, moment(5), moment(7)).is_err());
    let receipt = super::super::write_receipt::WriteReceipt::new(1, 64);
    receipt.revoked(1)?;
    parent.challenge_revoked(original.key, 1)?;
    assert!(parent.retired_challenges.iter().all(Option::is_none));
    assert!(parent.consume_ui_response(response(original)?, original.key.id, moment(8)).is_err());
    assert!(parent.challenge_issued(original.key, 1, moment(5), moment(8)).is_err());
    Ok(())
}
#[test]
fn activity_before_delayed_native_issue_observation_keeps_original_operation_and_cutoff()
-> TestResult {
    let mut parent = actor()?;
    let original = queue(&mut parent, 5, 1)?;
    parent.ui_activity(UiWatchdogActivity::Inactive, moment(6))?;
    let before = parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot();
    issue(&mut parent, original, 5, 7)?;
    let retained =
        parent.retired_challenges.iter().flatten().next().ok_or("Retired record missing")?;
    assert_eq!(retained.operation, original.operation);
    assert_eq!(retained.until, original.until);
    assert_eq!(retained.header, original.header);
    assert_eq!(parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot(), before);
    parent.consume_ui_response(response(original)?, original.key.id, moment(8))?;
    assert_eq!(parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot(), before);
    Ok(())
}
#[test]
fn changed_binding_key_counter_and_expired_original_response_preserve_retired_record() -> TestResult
{
    let mut parent = actor()?;
    let original = queue(&mut parent, 5, 1)?;
    issue(&mut parent, original, 5, 5)?;
    parent.ui_activity(UiWatchdogActivity::Inactive, moment(6))?;
    let before = parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot();
    let mut header = response(original)?;
    header.run.instance = sugarsubstitute_contracts::lifecycle::InstanceId::new(2)?;
    assert!(parent.consume_ui_response(header, original.key.id, moment(7)).is_err());
    let mut header = response(original)?;
    header.connection = Some(ConnectionId::new(2)?);
    assert!(parent.consume_ui_response(header, original.key.id, moment(7)).is_err());
    let mut header = response(original)?;
    header.deadline_qpc += 1;
    assert!(parent.consume_ui_response(header, original.key.id, moment(7)).is_err());
    let mut header = response(original)?;
    header.request = None;
    assert!(parent.consume_ui_response(header, original.key.id, moment(7)).is_err());
    assert!(
        parent.consume_ui_response(response(original)?, UiChallengeId::new(2)?, moment(7)).is_err()
    );
    let expired = parent.consume_ui_response(response(original)?, original.key.id, original.until);
    assert!(expired.as_ref().err().is_some_and(|cause| cause.downcast_ref::<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError>() == Some(&sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError::DeadlineExpired)));
    assert_eq!(
        parent.retired_challenges.iter().flatten().next().map(|challenge| challenge.until),
        Some(original.until)
    );
    assert_eq!(parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot(), before);
    parent.consume_ui_response(response(original)?, original.key.id, moment(34))?;
    Ok(())
}
#[test]
fn retired_response_cannot_clear_or_renew_a_new_current_challenge() -> TestResult {
    let mut parent = actor()?;
    let original = queue(&mut parent, 5, 1)?;
    issue(&mut parent, original, 5, 5)?;
    parent.ui_activity(UiWatchdogActivity::Inactive, moment(6))?;
    parent.ui_activity(UiWatchdogActivity::VisibleActive, moment(7))?;
    let current = queue(&mut parent, 12, 2)?;
    issue(&mut parent, current, 12, 12)?;
    let before = parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot();
    assert_eq!(before.pending, Some(current.key));
    assert_eq!(before.phase, Some(UiChallengePhase::Dispatched));
    parent.consume_ui_response(response(original)?, original.key.id, moment(13))?;
    assert_eq!(parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot(), before);
    assert_eq!(
        parent.challenge.map(|challenge| (challenge.key, challenge.header, challenge.until)),
        Some((current.key, current.header, current.until))
    );
    parent.consume_ui_response(response(current)?, current.key.id, moment(14))?;
    assert!(parent.challenge.is_none());
    Ok(())
}
#[test]
fn queued_revocation_retries_same_cutoff_and_expired_retired_metadata_never_reopens_progress()
-> TestResult {
    let mut parent = actor()?;
    let original = queue(&mut parent, 5, 1)?;
    parent.challenge_revoked(original.key, 1)?;
    parent.queue_challenge_write(original.key, 2)?;
    let retried = parent.challenge.ok_or("Original record lost")?;
    assert_eq!(retried.header, original.header);
    assert_eq!(retried.until, original.until);
    assert_eq!(retried.operation, Some(2));
    issue(&mut parent, retried, 6, 6)?;
    parent.ui_activity(UiWatchdogActivity::Inactive, moment(7))?;
    parent.expire_retired_challenges(moment(34));
    assert_eq!(parent.retired_challenges.iter().flatten().count(), 1);
    parent.expire_retired_challenges(moment(35));
    assert!(parent.retired_challenges.iter().all(Option::is_none));
    assert!(parent.consume_ui_response(response(original)?, original.key.id, moment(35)).is_err());
    assert_eq!(
        parent.watchdog.as_ref().ok_or("Watchdog missing")?.snapshot().activity,
        UiWatchdogActivity::Inactive
    );
    Ok(())
}
