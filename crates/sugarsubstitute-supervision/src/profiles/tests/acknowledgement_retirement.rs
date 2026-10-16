// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise cached profile responses through the actual parent correlation owner.
use super::super::*;
use super::support::{Harness, TestResult};
use crate::backend_custody_parent::observation_fixture;
use sugarsubstitute_contracts::{backend_custody::*, execution::*};

fn parent(
    h: &mut Harness,
    scope: u64,
    session: u64,
) -> Result<(crate::BackendCustodyParent, Scope), Box<dyn std::error::Error>> {
    let native = h
        .client
        .create_native_scope(ScopeSpec { id: ScopeId::new(scope)?, deadline: None }, None)?;
    let mut spec = h.spec(100)?;
    spec.class = JobClass::NativeObservation;
    spec.deadline = None;
    spec.reservation_bytes =
        sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES;
    let parent = observation_fixture::parent(
        &h.client,
        &native,
        spec,
        BackendCustodyKey::new(h.lease.run.owner, BackendCustodySession::new(session)?),
        Moment::from_nanos(30_000_000_000),
    )?;
    Ok((parent, native))
}

#[test]
fn original_published_receipt_can_drain_during_close_without_fresh_setup() -> TestResult {
    let mut h = Harness::new()?;
    let (parent, native) = parent(&mut h, 3, 3)?;
    let key = ProfileCustodyKey::new(h.lease.run, 2)?;
    observation_fixture::reply(
        &parent,
        4,
        BackendCustodyCommand::ProfilePublished { profile: key },
        BackendCustodyReceipt::ProfilePublished(key),
    )?;
    parent.close()?;
    let mut ack = None;
    custody::acknowledgement::published(&parent, key, Some(4), &mut ack)?;
    let actual = ack.as_ref().ok_or("Original response not consumed")?;
    assert_eq!(actual.nonce(), 4);
    assert_eq!(actual.receipt(), BackendCustodyReceipt::ProfilePublished(key));
    assert!(parent.take_acknowledgement_for(4).is_none());
    custody::acknowledgement::published(&parent, key, Some(4), &mut ack)?;
    assert!(native.snapshot().reserved_bytes > 0);
    drop(ack);
    drop(parent);
    h.finish_workers();
    assert_eq!(native.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn foreign_nonce_stays_pending_and_same_key_other_parent_cannot_replace_source() -> TestResult {
    let mut h = Harness::new()?;
    let (first, _) = parent(&mut h, 3, 4)?;
    let (second, _) = parent(&mut h, 4, 4)?;
    let key = ProfileCustodyKey::new(h.lease.run, 2)?;
    observation_fixture::reply(
        &first,
        4,
        BackendCustodyCommand::ProfilePublished { profile: key },
        BackendCustodyReceipt::ProfilePublished(key),
    )?;
    let mut ack = None;
    custody::acknowledgement::published(&first, key, Some(5), &mut ack)?;
    assert!(ack.is_none());
    custody::acknowledgement::published(&first, key, Some(4), &mut ack)?;
    assert!(ack.is_some());
    assert!(custody::acknowledgement::published(&second, key, Some(4), &mut ack).is_err());
    assert!(ack.is_some());
    first.close()?;
    second.close()?;
    drop(ack);
    drop(first);
    drop(second);
    h.finish_workers();
    Ok(())
}

#[test]
fn original_prepared_reply_drains_after_close_without_granting_new_setup() -> TestResult {
    let mut h = Harness::new()?;
    let (parent, native) = parent(&mut h, 3, 5)?;
    let key = ProfileCustodyKey::new(h.lease.run, 2)?;
    // External encoded matching facts do not create native output cells or helper authority.
    observation_fixture::reply(
        &parent,
        3,
        BackendCustodyCommand::ProfilePrepare {
            recovery_grant: None,
            profile: key,
            job: 40,
            directory: 44,
            record: [8; 160],
            claim_identity: [6; 24],
            until_ticks: 100,
            frequency: 10,
        },
        BackendCustodyReceipt::ProfilePrepared(key),
    )?;
    let mut ack = None;
    custody::acknowledgement::prepared_during_close(
        &parent,
        key,
        Some(3),
        &h.lease.shared.closing,
        &mut ack,
    )?;
    assert!(ack.is_none(), "Ordinary setup's Prepared response was taken by cleanup");
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    parent.close()?;
    custody::acknowledgement::prepared_during_close(
        &parent,
        key,
        Some(3),
        &h.lease.shared.closing,
        &mut ack,
    )?;
    assert_eq!(
        ack.as_ref().ok_or("Original Prepared lost")?.receipt(),
        BackendCustodyReceipt::ProfilePrepared(key)
    );
    assert!(parent.take_acknowledgement_for(3).is_none());
    assert!(native.snapshot().reserved_bytes > 0);
    custody::acknowledgement::prepared_during_close(
        &parent,
        key,
        Some(3),
        &h.lease.shared.closing,
        &mut ack,
    )?;
    let actual = h.lease.consume(&batch)?.ok_or("Original cleanup result lost")?;
    assert_eq!(actual.job, release.id);
    assert_eq!(actual.kind, TerminalKind::Success);
    assert!(actual.failure.is_none());
    assert_eq!(h.lease.release_deadline, Some(Moment::from_nanos(100)));
    drop(ack);
    drop(parent);
    h.finish_workers();
    assert_eq!(native.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn wrong_profile_reply_purpose_retains_actual_original_guard() -> TestResult {
    let mut h = Harness::new()?;
    h.release(100)?;
    let (parent, _) = parent(&mut h, 3, 6)?;
    let key = ProfileCustodyKey::new(h.lease.run, 2)?;
    observation_fixture::reply(
        &parent,
        4,
        BackendCustodyCommand::ProfilePublished { profile: key },
        BackendCustodyReceipt::ProfilePublished(key),
    )?;
    let mut ack = None;
    assert!(
        custody::acknowledgement::prepared_during_close(
            &parent,
            key,
            Some(4),
            &h.lease.shared.closing,
            &mut ack
        )
        .is_err()
    );
    assert_eq!(
        ack.as_ref().ok_or("Original guard discarded")?.receipt(),
        BackendCustodyReceipt::ProfilePublished(key)
    );
    custody::acknowledgement::published(&parent, key, Some(4), &mut ack)?;
    parent.close()?;
    drop(ack);
    drop(parent);
    h.finish_workers();
    Ok(())
}

#[test]
fn actual_completed_profile_close_rejects_fresh_publication_before_setup_effects() -> TestResult {
    let mut h = Harness::new()?;
    let actual = h.release(100)?;
    assert_eq!(actual.kind, TerminalKind::Success);
    assert!(h.lease.snapshot().retirement_ready());
    let spec = h.spec(150)?;
    assert!(matches!(
        h.lease.submit_custody_publication(&h.client, &h.scope, spec),
        Err(ClaimAdmissionError::Closed)
    ));
    assert!(!h.lease.snapshot().pending);
    assert_eq!(h.lease.release_deadline, Some(Moment::from_nanos(100)));
    h.finish_workers();
    Ok(())
}
