// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual Parent admission ledger after its real accepted exit transaction.
use super::*;
use crate::production::parent_lifecycle::history::{HeldBackendCommand, HeldBackendTerminal};
use sugarsubstitute_application::supervision::ResourceCoordinator;
use sugarsubstitute_contracts::configuration::LoopbackOrigin;

/// Keep external decoded input in the production request ledger with its original cutoff.
fn hold(parent: &mut ParentLifecycle, command: HeldBackendCommand) -> Result<(usize, WireHeader)> {
    let until = parent.cleanup.ok_or("Original role cleanup missing")?.settle_until;
    let original = WireHeader { deadline_qpc: i64::try_from(until.as_nanos())?, ..header(9)? };
    let (index, fresh) = parent.requests.admit(original, b"actual retained logical command")?;
    assert!(fresh);
    parent.requests.get_mut(index)?.until = Some(until);
    parent.hold_backend_command(index, command).map_err(|cause| cause as Box<dyn Error>)?;
    Ok((index, original))
}

/// Invoke the actual production admission owner, preserving its returned command and original cause.
fn next(
    parent: &mut ParentLifecycle,
    now: Moment,
    pending: bool,
    open: bool,
) -> Result<Option<(usize, HeldBackendCommand)>> {
    parent.next_backend_command(now, pending, open).map_err(|cause| cause as Box<dyn Error>)
}

/// Use the real attached-lease policy; no readiness/native ownership proof is manufactured here.
fn attached(parent: &ParentLifecycle) -> Result<(ResourceCoordinator, LeaseKey)> {
    let mut policy = ResourceCoordinator::new(parent.run.owner);
    let profile = ProfileId::new("Close_1".into())?;
    policy.enroll_run(RunEnrollment {
        run: parent.run,
        launch: parent.launch,
        profile: profile.clone(),
    })?;
    policy.register_run(ClientRegistration {
        run: parent.run,
        connection: parent.connection,
        launch: parent.launch,
        profile,
    })?;
    let binding = ClientBinding { run: parent.run, connection: parent.connection };
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8189", 8189)?;
    let acquired = policy.request(
        binding,
        ResourceRequest {
            run: parent.run,
            connection: parent.connection,
            id: RequestId::new(1)?,
            command: ResourceCommand::Acquire(BackendTarget::Attached(origin)),
        },
    )?;
    let lease = match acquired.replies.first().map(|reply| reply.outcome) {
        Some(RequestOutcome::Pending(lease)) => lease,
        _ => return Err("Actual attached pending lease missing".into()),
    };
    policy
        .observe(ResourceObservation::Ready { resource: lease.resource, origin })
        .map_err(|rejected| rejected.reason)?;
    Ok((policy, lease))
}

#[test]
fn exit_intent_release_reaches_actual_attached_policy_before_any_physical_retirement() -> Result<()>
{
    let mut parent = actor()?;
    intent(&mut parent, CompletionIntent::Normal)?;
    let original_budget = parent.cleanup;
    let (mut policy, lease) = attached(&parent)?;
    let (index, original) = hold(&mut parent, HeldBackendCommand::Release(lease))?;
    parent.forwarded = Some(ResourceTransition::default());
    assert!(next(&mut parent, Moment::from_nanos(2), false, false)?.is_none());
    parent.forwarded = None;
    assert!(next(&mut parent, Moment::from_nanos(2), true, false)?.is_none());
    let (selected, command) = next(&mut parent, Moment::from_nanos(2), false, false)?
        .ok_or("ExitIntent cancelled the original Release")?;
    assert_eq!(selected, index);
    let HeldBackendCommand::Release(actual) = command else {
        return Err("Wrong admitted command".into());
    };
    assert_eq!(actual, lease);
    let reply = policy.request(
        ClientBinding { run: parent.run, connection: parent.connection },
        ResourceRequest {
            run: parent.run,
            connection: parent.connection,
            id: original.request.ok_or("Original request missing")?,
            command: ResourceCommand::Release(actual),
        },
    )?;
    assert_eq!(reply.replies.len(), 1);
    assert_eq!(reply.replies[0].outcome, RequestOutcome::Released);
    assert!(
        matches!(reply.effects.as_slice(), [ResourceEffect::DisconnectAttached { resource }] if *resource == lease.resource)
    );
    assert_eq!(policy.snapshot().leases, 0);
    assert_eq!(policy.snapshot().resources, 1, "Release falsely claimed physical settlement");
    assert!(!parent.closed);
    assert!(!parent.native_retired());
    assert_eq!(parent.cleanup, original_budget);
    assert_eq!(parent.requests.get(index)?.header, original);
    assert_eq!(
        parent.requests.get(index)?.until,
        original_budget.map(|budget| budget.settle_until)
    );
    assert_eq!(parent.requests.get(index)?.held_terminal, None);
    assert!(next(&mut parent, Moment::from_nanos(3), false, false)?.is_none());
    Ok(())
}

#[test]
fn exit_intent_cancels_fresh_work_and_physical_channel_close_cancels_release() -> Result<()> {
    let id = identity()?;
    let mut prototype = actor()?;
    intent(&mut prototype, CompletionIntent::Normal)?;
    let (_, lease) = attached(&prototype)?;
    for command in [
        HeldBackendCommand::Acquire(id),
        HeldBackendCommand::Retry {
            identity: id,
            failed: super::super::super::replies::reference(lease),
        },
        HeldBackendCommand::Reload { identity: id, lease },
        HeldBackendCommand::Release(lease),
    ] {
        let mut parent = actor()?;
        intent(&mut parent, CompletionIntent::Normal)?;
        parent.closed = matches!(command, HeldBackendCommand::Release(_));
        let (index, original) = hold(&mut parent, command)?;
        assert!(next(&mut parent, Moment::from_nanos(2), false, true)?.is_none());
        assert_eq!(parent.requests.get(index)?.held_terminal, Some(HeldBackendTerminal::Cancelled));
        assert_eq!(parent.requests.get(index)?.header, original);
        assert!(parent.requests.get(index)?.reply.is_none());
    }
    Ok(())
}

#[test]
fn exit_intent_release_keeps_exact_original_deadline_before_and_at_expiry() -> Result<()> {
    let mut parent = actor()?;
    intent(&mut parent, CompletionIntent::Normal)?;
    let (_, lease) = attached(&parent)?;
    let (index, original) = hold(&mut parent, HeldBackendCommand::Release(lease))?;
    let until = parent.cleanup.ok_or("Original cleanup missing")?.settle_until;
    let before = Moment::from_nanos(until.as_nanos() - 1);
    let payload = sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload::Release(
        super::super::super::replies::reference(lease),
    );
    let after = Moment::from_nanos(until.as_nanos() + 1);
    assert_eq!(parent.cleanup_control_deadline(&payload, after), until);
    assert_eq!(parent.cleanup_control_deadline(&payload, before), before);
    assert!(next(&mut parent, before, true, false)?.is_none());
    assert_eq!(parent.requests.get(index)?.held_terminal, None);
    assert!(parent.requests.get(index)?.held_backend.is_some());
    assert!(next(&mut parent, until, false, false)?.is_none());
    assert_eq!(parent.requests.get(index)?.held_terminal, Some(HeldBackendTerminal::TimedOut));
    assert_eq!(parent.requests.get(index)?.header, original);
    assert_eq!(parent.requests.get(index)?.until, Some(until));
    assert!(parent.requests.get(index)?.reply.is_none());
    assert!(next(&mut parent, until, false, false)?.is_none());
    Ok(())
}
