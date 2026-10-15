// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual native binding-ledger transitions; fake only producer facts and App acceptance.
//! These tests confer no native readiness, transport receipt, source lease or process authority.
use super::*;
use std::cell::Cell;
use sugarsubstitute_contracts::lifecycle::{
    ConnectionId, LeaseId, ResourceGeneration, ResourceId, ResourceKey, ResourceTransition,
};
use sugarsubstitute_supervision::lifecycle_protocol::LeaseReference;
type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;
fn lease(generation: u64) -> Result<LeaseKey> {
    let identity = super::super::tests::identity().map_err(|cause| cause.to_string())?;
    Ok(LeaseKey {
        run: identity.run,
        connection: ConnectionId::new(1)?,
        id: LeaseId::new(generation)?,
        resource: ResourceKey {
            owner: identity.run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(generation)?,
        },
    })
}
fn document(port: u16) -> Result<MountedDocument> {
    MountedDocument::new(
        LoopbackOrigin::parse(&format!("http://127.0.0.1:{port}"), port)?,
        "1.2.3",
        "4.5.6",
    )
}
fn initial() -> Result<(Recovery, LeaseKey, LeaseKey)> {
    let a = lease(1)?;
    let b = lease(2)?;
    let mut owner = Recovery::default();
    owner.bind_initial(a, document(8188)?)?;
    Ok((owner, a, b))
}
fn header(lease: LeaseKey, request: u64) -> Result<WireHeader> {
    Ok(WireHeader {
        run: lease.run,
        connection: Some(lease.connection),
        request: Some(sugarsubstitute_contracts::lifecycle::RequestId::new(request)?),
        deadline_qpc: 100,
    })
}
fn death(lease: LeaseKey) -> GrantedResourceInvalidation {
    GrantedResourceInvalidation { lease, status: 259 }
}
fn replacement(previous: LeaseKey, current: LeaseKey) -> GrantedResourceReplacement {
    GrantedResourceReplacement { previous, current }
}
fn pending(owner: &mut Recovery, a: LeaseKey, b: LeaseKey) -> Result<WireHeader> {
    owner.invalidate(death(a), 2)?;
    owner.replace(replacement(a, b))?;
    let header = header(a, 3)?;
    owner.stage(header)?;
    Ok(header)
}
#[test]
fn exact_historical_handoff_needs_written_then_current_pull_and_external_adoption() -> TestResult {
    let (mut owner, a, b) = initial()?;
    let original = owner.running.ok_or(ParentError::WrongPhase)?.document;
    let header = pending(&mut owner, a, b)?;
    let calls = Cell::new(0);
    assert!(
        owner
            .consume_binding(b, None, |_, _| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(owner.handoff.is_some());
    assert_eq!(owner.running.ok_or(ParentError::WrongPhase)?.lease, a);
    owner.written(header)?;
    assert_eq!(owner.running.ok_or(ParentError::WrongPhase)?.lease, a);
    owner.consume_binding(b, None, |pair, notice| {
        assert_eq!(pair, replacement(a, b));
        assert!(notice.is_none());
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 1);
    assert!(owner.handoff.is_none());
    assert!(owner.outage.is_none());
    assert_eq!(owner.running.ok_or(ParentError::WrongPhase)?.lease, b);
    assert!(owner.running.ok_or(ParentError::WrongPhase)?.document == original);
    assert!(owner.consume_binding(b, None, |_, _| Err("must not run twice".into())).is_err());
    Ok(())
}
#[test]
fn old_ack_foreign_current_and_external_rejection_preserve_original_handoff() -> TestResult {
    let (mut owner, a, b) = initial()?;
    let header = pending(&mut owner, a, b)?;
    owner.written(header)?;
    let calls = Cell::new(0);
    assert!(
        owner
            .consume_binding(b, std::num::NonZeroU64::new(1), |_, _| {
                calls.set(1);
                Ok(())
            })
            .is_err()
    );
    assert!(
        owner
            .consume_binding(lease(3)?, None, |_, _| {
                calls.set(1);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(
        owner
            .consume_binding(b, None, |_, _| Err("actual App/live source rejection".into()))
            .is_err()
    );
    let handoff = owner.handoff.ok_or(ParentError::WrongPhase)?;
    assert_eq!(handoff.pair, replacement(a, b));
    assert!(handoff.written);
    assert_eq!(handoff.header, Some(header));
    assert_eq!(owner.running.ok_or(ParentError::WrongPhase)?.lease, a);
    assert_eq!(owner.outage.ok_or(ParentError::WrongPhase)?.failed, a);
    Ok(())
}
#[test]
fn successor_death_before_or_after_write_keeps_actual_notice_through_current_binding_consumption()
-> TestResult {
    for before_write in [true, false] {
        let (mut owner, a, b) = initial()?;
        let header = pending(&mut owner, a, b)?;
        if before_write {
            owner.invalidate(death(b), 2)?;
        }
        owner.written(header)?;
        if !before_write {
            owner.invalidate(death(b), 2)?;
        }
        let calls = Cell::new(0);
        owner.consume_binding(b, None, |pair, notice| {
            assert_eq!(pair, replacement(a, b));
            assert_eq!(notice, Some(death(b)));
            // External adapter must independently validate B and absent A; no live adoption is requested here.
            calls.set(calls.get() + 1);
            Ok(())
        })?;
        assert_eq!(calls.get(), 1);
        assert_eq!(owner.running.ok_or(ParentError::WrongPhase)?.lease, b);
        assert_eq!(owner.outage.ok_or(ParentError::WrongPhase)?.failed, b);
        assert_eq!(owner.live_lease(), None);
        assert!(owner.handoff.is_none());
    }
    Ok(())
}
#[test]
fn complete_transition_validates_real_event_context_before_any_owner_mutation() -> TestResult {
    let (owner, a, b) = initial()?;
    let transition = ResourceTransition {
        invalidations: vec![death(a), death(b)],
        replacements: vec![replacement(a, b)],
        ..ResourceTransition::default()
    };
    assert!(owner.transition(a.run, &transition, |_| document(8199), |_| Ok(2)).is_err());
    assert!(owner.outage.is_none());
    assert!(owner.handoff.is_none());
    let next = owner.transition(a.run, &transition, |_| document(8188), |_| Ok(2))?;
    assert_eq!(next.outage.ok_or(ParentError::WrongPhase)?.status, 259);
    assert_eq!(next.handoff.ok_or(ParentError::WrongPhase)?.next, Some(death(b)));
    assert_eq!(next.live_lease(), None);
    assert_eq!(next.running.ok_or(ParentError::WrongPhase)?.lease, a);
    Ok(())
}
#[test]
fn second_pair_and_changed_death_backpressure_preserve_first_pair_and_cause_identity() -> TestResult
{
    let (mut owner, a, b) = initial()?;
    pending(&mut owner, a, b)?;
    let changed = GrantedResourceInvalidation { status: 1, ..death(a) };
    assert!(owner.invalidate(changed, 2).is_err());
    let cause = owner.replace(replacement(a, lease(3)?)).err().ok_or("Second pair should wait")?;
    assert_eq!(cause.downcast_ref::<ParentError>(), Some(&ParentError::TransitionPending));
    assert_eq!(owner.handoff.ok_or(ParentError::WrongPhase)?.pair, replacement(a, b));
    assert_eq!(owner.outage.ok_or(ParentError::WrongPhase)?.status, 259);
    assert!(owner.stage(header(a, 4)?).is_err());
    let wrong = header(a, 4)?;
    assert!(owner.written(wrong).is_err());
    assert!(!owner.handoff.ok_or(ParentError::WrongPhase)?.written);
    Ok(())
}
#[test]
fn mounted_context_and_lease_lineage_reject_foreign_selection_generation_and_zero_count()
-> TestResult {
    let (mut owner, a, b) = initial()?;
    assert!(owner.invalidate(death(a), 0).is_err());
    assert!(owner.invalidate(death(a), 65).is_err());
    owner.invalidate(death(a), 1)?;
    let foreign = LeaseKey { connection: ConnectionId::new(2)?, ..b };
    assert!(owner.replace(replacement(a, foreign)).is_err());
    assert!(owner.replace(replacement(a, a)).is_err());
    assert!(owner.handoff.is_none());
    assert!(
        MountedDocument::new(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?, "", "ok")
            .is_err()
    );
    assert_eq!(
        super::super::replies::reference(a),
        LeaseReference { resource: a.resource.id, generation: a.resource.generation, lease: a.id }
    );
    Ok(())
}
#[test]
fn original_held_ledger_command_opens_once_without_blocking_other_request_admission() -> TestResult
{
    use super::super::history::{HeldBackendCommand, Requests};
    let a = lease(1)?;
    let mut requests = Requests::new();
    let original = header(a, 1)?;
    let (index, _) = requests.admit(original, b"actual original Retry frame digest")?;
    requests.get_mut(index)?.until = Some(Moment::from_nanos(100));
    let identity = super::super::tests::identity().map_err(|cause| cause.to_string())?;
    requests.get_mut(index)?.held_backend =
        Some(HeldBackendCommand::Retry { identity, failed: super::super::replies::reference(a) });
    assert!(requests.poll_held_backend(Moment::from_nanos(1), false)?.is_none());
    let (exit, _) = requests.admit(header(a, 2)?, b"separate authenticated ExitIntent")?;
    requests.get_mut(exit)?.reply = Some(Reply::Acknowledged);
    assert_eq!(requests.next_reply(), Some(exit));
    assert!(!requests.admit(original, b"actual original Retry frame digest")?.1);
    let (resumed, command) = requests
        .poll_held_backend(Moment::from_nanos(99), true)?
        .ok_or("Original command missing")?;
    assert_eq!(resumed, index);
    assert!(
        matches!(command,HeldBackendCommand::Retry { failed,.. } if failed==super::super::replies::reference(a))
    );
    assert_eq!(requests.get(index)?.header, original);
    assert_eq!(requests.get(index)?.until, Some(Moment::from_nanos(100)));
    assert!(requests.poll_held_backend(Moment::from_nanos(99), true)?.is_none());
    Ok(())
}
#[test]
fn held_timeout_at_original_cutoff_never_admits_effect_or_renews_header() -> TestResult {
    use super::super::history::{HeldBackendCommand, HeldBackendTerminal, Requests};
    let a = lease(1)?;
    let mut requests = Requests::new();
    let original = header(a, 1)?;
    let (index, _) = requests.admit(original, b"original Acquire")?;
    requests.get_mut(index)?.until = Some(Moment::from_nanos(100));
    requests.get_mut(index)?.held_backend = Some(HeldBackendCommand::Acquire(
        super::super::tests::identity().map_err(|cause| cause.to_string())?,
    ));
    assert!(requests.poll_held_backend(Moment::from_nanos(100), true)?.is_none());
    assert_eq!(requests.get(index)?.held_terminal, Some(HeldBackendTerminal::TimedOut));
    assert_eq!(requests.get(index)?.header, original);
    assert_eq!(requests.get(index)?.until, Some(Moment::from_nanos(100)));
    assert!(requests.poll_held_backend(Moment::from_nanos(101), true)?.is_none());
    assert!(requests.next_reply().is_none());
    assert!(!requests.admit(original, b"original Acquire")?.1);
    Ok(())
}

#[test]
fn native_reload_owner_preserves_creation_effects_and_rejects_successor_before_any_effect()
-> TestResult {
    let (mut owner, a, b) = initial()?;
    let backend = Cell::new(a);
    let main = Cell::new(1_u32);
    let calls = Cell::new(0);
    assert_eq!(
        owner.reload(Some(a), super::super::replies::reference(a), |lease| {
            assert_eq!(lease, a);
            calls.set(calls.get() + 1);
            main.set(2);
            Ok(())
        })?,
        Some(())
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(backend.get(), a);
    assert_eq!(main.get(), 2);
    let h = pending(&mut owner, a, b)?;
    owner.written(h)?;
    owner.consume_binding(b, None, |_, _| Ok(()))?;
    backend.set(b);
    assert_eq!(
        owner.reload(Some(a), super::super::replies::reference(b), |_| {
            calls.set(calls.get() + 1);
            backend.set(a);
            main.set(3);
            Ok(())
        })?,
        None
    );
    assert!(
        owner
            .reload(Some(a), super::super::replies::reference(a), |_| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(backend.get(), b);
    assert_eq!(main.get(), 2);
    assert_eq!(owner.running.ok_or(ParentError::WrongPhase)?.lease, b);
    Ok(())
}
#[test]
fn deliberate_close_cancels_only_unadmitted_command_and_keeps_exit_reply_live() -> TestResult {
    use super::super::history::{HeldBackendCommand, HeldBackendTerminal, Requests};
    let a = lease(1)?;
    let mut requests = Requests::new();
    let (held, _) = requests.admit(header(a, 1)?, b"original held Acquire")?;
    requests.get_mut(held)?.until = Some(Moment::from_nanos(100));
    requests.get_mut(held)?.held_backend = Some(HeldBackendCommand::Acquire(
        super::super::tests::identity().map_err(|cause| cause.to_string())?,
    ));
    let (exit, _) = requests.admit(header(a, 2)?, b"actual ExitIntent logical boundary")?;
    requests.get_mut(exit)?.reply = Some(Reply::Acknowledged);
    requests.cancel_held(held)?;
    assert_eq!(requests.get(held)?.held_terminal, Some(HeldBackendTerminal::Cancelled));
    assert_eq!(requests.next_reply(), Some(exit));
    assert!(requests.poll_held_backend(Moment::from_nanos(10), true)?.is_none());
    Ok(())
}
