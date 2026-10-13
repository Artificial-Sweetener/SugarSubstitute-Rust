// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact reply routing through actual wire/ledger rules using inert server bytes.
use super::*;
use crate::production::child::{handshake::WireHandshake, requests::ClientRequests};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::CounterDeadline,
    lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, LeaseId, NamespaceId,
        RequestId, ResourceGeneration, ResourceId, RunGeneration,
    },
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    GrantedContext, LeaseReference, VersionLabel,
};

/// Fake server messages exercise wire order only; no authenticated peer or native frame exists.
fn requests() -> std::result::Result<ClientRequests, Box<dyn Error>> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(9)?,
    };
    let mut binding = WireHandshake::new(run, CounterDeadline::new(500, 100)?);
    binding.hello_admitted()?;
    let header = WireHeader {
        run,
        connection: Some(ConnectionId::new(19)?),
        request: None,
        deadline_qpc: 500,
    };
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(
        &WireMessage { header, payload: WirePayload::Welcome { qpc_frequency: 100 } },
        &mut bytes,
    )?;
    binding.welcome(&FrameCodec::decode(&bytes[..count])?)?;
    let request = RequestId::new(23)?;
    binding.register_admitted(request)?;
    let count = FrameCodec::encode(
        &WireMessage {
            header: WireHeader { request: Some(request), ..header },
            payload: WirePayload::Reply(ReplyOutcome::Acknowledged),
        },
        &mut bytes,
    )?;
    binding.registered(&FrameCodec::decode(&bytes[..count])?)?;
    Ok(ClientRequests::new(&binding)?)
}

/// A control ACK stays with its ledger owner before the exact Acquire reply can transfer.
#[test]
fn activity_ack_before_acquire_grant_keeps_original_owner_and_cutoffs()
-> std::result::Result<(), Box<dyn Error>> {
    let mut requests = requests()?;
    let activity_until = Moment::from_nanos(500);
    let acquire_until = Moment::from_nanos(1000);
    let activity = requests.prepare(activity_until, CounterDeadline::new(1000, 100)?)?;
    let acquire = requests.prepare(acquire_until, CounterDeadline::new(2000, 100)?)?;
    requests.outbound(activity, activity_until, Moment::from_nanos(10))?;
    requests.admitted(activity)?;
    requests.outbound(acquire, acquire_until, Moment::from_nanos(10))?;
    requests.admitted(acquire)?;
    let pending = PendingAcquire { header: acquire, original: acquire_until, admitted: true };
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(
        &WireMessage { header: activity, payload: WirePayload::Reply(ReplyOutcome::Acknowledged) },
        &mut bytes,
    )?;
    let observed = FrameCodec::decode(&bytes[..count])?;
    requests.received(observed.header, Moment::from_nanos(20))?;
    let mut takes = 0;
    assert!(
        pending
            .take_matching(Some(observed.header), || {
                takes += 1;
                Some(&bytes[..count])
            })
            .is_none()
    );
    assert_eq!(takes, 0);
    requests.consumed(activity)?;
    assert!(requests.consumed(acquire).is_err());
    let identity = MainIdentity {
        run: acquire.run,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    // GrantedContext is expressly inert wire shape, never a native/backend readiness proof.
    let count = FrameCodec::encode(
        &WireMessage {
            header: acquire,
            payload: WirePayload::Reply(ReplyOutcome::Granted(GrantedContext {
                lease: LeaseReference {
                    resource: ResourceId::new(1)?,
                    generation: ResourceGeneration::new(1)?,
                    lease: LeaseId::new(1)?,
                },
                identity,
                origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
                frontend: VersionLabel::new("1.0")?,
                sugarcubes: VersionLabel::new("1.0")?,
            })),
        },
        &mut bytes,
    )?;
    let observed = FrameCodec::decode(&bytes[..count])?;
    requests.received(observed.header, Moment::from_nanos(30))?;
    assert!(
        pending
            .take_matching(
                Some(WireHeader { deadline_qpc: acquire.deadline_qpc + 1, ..acquire }),
                || {
                    takes += 1;
                    Some(&bytes[..count])
                }
            )
            .is_none()
    );
    let transferred = pending
        .take_matching(Some(observed.header), || {
            takes += 1;
            Some(&bytes[..count])
        })
        .ok_or("Exact Acquire reply not selected")?;
    let decoded = FrameCodec::decode(transferred)?;
    assert_eq!(decoded.header, acquire);
    assert!(
        matches!(decoded.payload, DecodedPayload::Reply(ReplyOutcome::Granted(grant)) if grant.identity == identity)
    );
    assert_eq!(takes, 1);
    assert_eq!(pending.original, acquire_until);
    assert_eq!(pending.header.deadline_qpc, 2000);
    requests.consumed(acquire)?;
    assert!(requests.consumed(activity).is_err());
    Ok(())
}
