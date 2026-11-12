// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual codec/request/source funding regressions; no fabricated native frames or peers.
use super::*;
use sugarsubstitute_contracts::startup::{LaunchId, MainIdentity};
use sugarsubstitute_supervision::lifecycle_protocol::{DecodedPayload, ReplyOutcome};
fn main_identity(binding: OutputBinding) -> TestResult<MainIdentity> {
    Ok(MainIdentity {
        run: binding.run,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn classified(
    owner: &OutputOwner,
    h: WireHeader,
    reply: ReplyOutcome<'_>,
) -> TestResult<Option<BackendNotice>> {
    let mut bytes = [0; 256];
    let count = FrameCodec::encode(
        &WireMessage { header: h, payload: WirePayload::Reply(reply) },
        &mut bytes,
    )?;
    let decoded = FrameCodec::decode(&bytes[..count])?;
    let DecodedPayload::Reply(reply) = decoded.payload else {
        return Err("Not a reply".into());
    };
    Ok(recovery::notice(
        owner.binding,
        owner.pending.ok_or("Original request absent")?,
        decoded.header,
        reply,
    )?)
}
#[test]
fn authenticated_notice_codec_rejects_pre_main_foreign_header_and_wrong_old_lease() -> TestResult {
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    let binding = binding()?;
    let h = header(binding, 2)?;
    owner.prepare_pull(h, Moment::from_nanos(100))?;
    let reply = ReplyOutcome::BackendUnavailable {
        failed: binding.lease,
        status: 259,
        affected_clients: 1,
        retry_ready: true,
    };
    assert!(classified(&owner, h, reply).is_err());
    drop(owner);
    drop(rig.owner);
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    owner.running_main(main_identity(binding)?)?;
    owner.prepare_pull(h, Moment::from_nanos(100))?;
    assert!(matches!(
        classified(&owner, h, reply)?,
        Some(BackendNotice::Unavailable { status: 259, .. })
    ));
    let foreign = WireHeader { request: Some(RequestId::new(3)?), ..h };
    assert!(classified(&owner, foreign, reply).is_err());
    let wrong = LeaseReference { lease: LeaseId::new(9)?, ..binding.lease };
    assert!(
        classified(
            &owner,
            h,
            ReplyOutcome::BackendUnavailable {
                failed: wrong,
                status: 0,
                affected_clients: 1,
                retry_ready: true
            }
        )
        .is_err()
    );
    drop(owner);
    drop(rig.owner);
    Ok(())
}
#[test]
fn retry_and_pull_each_accept_one_exact_replacement_without_reusing_old_ack() -> TestResult {
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    let binding = binding()?;
    let identity = main_identity(binding)?;
    owner.running_main(identity)?;
    let current = LeaseReference {
        resource: ResourceId::new(2)?,
        generation: ResourceGeneration::new(1)?,
        lease: LeaseId::new(2)?,
    };
    let h = header(binding, 2)?;
    owner.prepare_pull(h, Moment::from_nanos(100))?;
    let reply = ReplyOutcome::BackendReplaced { identity, previous: binding.lease, current };
    assert!(
        matches!(classified(&owner,h,reply)?,Some(BackendNotice::Replaced{current:value,..}) if value==current)
    );
    assert!(
        owner
            .prepare_retry(header(binding, 3)?, Moment::from_nanos(100), identity, binding.lease)
            .is_err()
    );
    drop(owner);
    drop(rig.owner);
    let rig = Rig::new()?;
    let mut owner = output(&rig)?;
    owner.running_main(identity)?;
    let message = owner.prepare_retry(
        header(binding, 3)?,
        Moment::from_nanos(100),
        identity,
        binding.lease,
    )?;
    assert!(
        matches!(message.payload,WirePayload::RetryBackend{failed,..} if failed==binding.lease)
    );
    assert!(classified(&owner, message.header, reply)?.is_some());
    assert!(classified(&owner, message.header, ReplyOutcome::OutputIdle(binding.lease)).is_err());
    assert!(owner.prepare_pull(header(binding, 4)?, Moment::from_nanos(100)).is_err());
    drop(owner);
    drop(rig.owner);
    Ok(())
}
#[test]
fn fresh_actual_source_admission_preserves_old_snapshot_credit_through_worker_join() -> TestResult {
    let rig = Rig::new()?;
    let old = output(&rig)?;
    let b = binding()?;
    let source = old.source.clone();
    let mut view_job = rig.client.submit_blocking(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| {
            let _scratch = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
            let state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            Ok::<_, OutputError>(OutputSnapshot::new(
                state.transcript.snapshot_tail(1)?,
                source.custody.clone(),
            ))
        },
    )?;
    let old_view = rig.result(&mut view_job)?;
    let resource = ResourceKey { id: ResourceId::new(2)?, ..b.resource };
    let next = OutputBinding {
        resource,
        lease: LeaseReference {
            resource: resource.id,
            generation: resource.generation,
            lease: LeaseId::new(2)?,
        },
        ..b
    };
    let mut constructor = OutputOwner::submit_new(
        &rig.client,
        &rig.scope,
        JobId::new(3)?,
        next,
        Moment::from_nanos(100),
    )?;
    let mut fresh = rig.result(&mut constructor)?;
    assert_eq!(old.binding, b);
    assert_eq!(fresh.binding, next);
    assert!(rig.scope.snapshot().reserved_bytes >= 2 * SOURCE_BYTES);
    let h = header(next, 4)?;
    let message = fresh.prepare_pull(h, Moment::from_nanos(100))?;
    assert!(
        matches!(message.payload,WirePayload::OutputPull{lease,acknowledged_delivery:None} if lease==next.lease)
    );
    assert_eq!(old_view.transcript().resource(), b.resource);
    drop((view_job, constructor, old, fresh));
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    drop(old_view);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_closed_scope_new_source_rejection_keeps_original_owner_and_credit() -> TestResult {
    let rig = Rig::new()?;
    let old = output(&rig)?;
    let binding = old.binding;
    rig.scope.cancel();
    assert!(
        OutputOwner::submit_new(
            &rig.client,
            &rig.scope,
            JobId::new(2)?,
            binding,
            Moment::from_nanos(100)
        )
        .is_err()
    );
    assert_eq!(old.binding, binding);
    assert!(rig.scope.snapshot().reserved_bytes >= SOURCE_BYTES);
    drop(old);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
