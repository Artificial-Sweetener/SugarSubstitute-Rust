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
            .take_matching(Moment::from_nanos(20), Some(observed.header), || {
                takes += 1;
                Some(&bytes[..count])
            })?
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
                Moment::from_nanos(30),
                Some(WireHeader { deadline_qpc: acquire.deadline_qpc + 1, ..acquire }),
                || {
                    takes += 1;
                    Some(&bytes[..count])
                }
            )?
            .is_none()
    );
    let transferred = pending
        .take_matching(Moment::from_nanos(30), Some(observed.header), || {
            takes += 1;
            Some(&bytes[..count])
        })?
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

/// Advance only the supplied clock; all worker lifetime remains with the actual executor.
struct AcquireClock(std::sync::atomic::AtomicU64);
impl sugarsubstitute_contracts::execution::Clock for AcquireClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(std::sync::atomic::Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> sugarsubstitute_contracts::execution::WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// The no-job boundary has no completion subscriber; it creates no additional scheduler.
struct AcquireWake;
impl sugarsubstitute_contracts::execution::CompletionWake for AcquireWake {
    fn wake(
        &self,
    ) -> std::result::Result<(), sugarsubstitute_contracts::execution::ExecutionError> {
        Ok(())
    }
}

/// The real admitted ledger remains intact when a quiet request reaches its original cutoff.
#[test]
fn quiet_admitted_acquire_expires_without_transferring_or_replacing_original_request()
-> std::result::Result<(), Box<dyn Error>> {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let clock = Arc::new(AcquireClock(AtomicU64::new(10)));
    let owner =
        sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), Arc::new(AcquireWake))?;
    let client = owner.client();
    let mut requests = requests()?;
    let until = Moment::from_nanos(1000);
    let header = requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    requests.outbound(header, until, client.now()?)?;
    requests.admitted(header)?;
    let pending = PendingAcquire { header, original: until, admitted: true };
    let mut effects = 0;
    assert!(
        pending
            .take_matching(client.now()?, None, || {
                effects += 1;
                Some(())
            })?
            .is_none()
    );
    clock.0.store(1000, Ordering::Release);
    let cause = pending
        .take_matching(client.now()?, None, || {
            effects += 1;
            Some(())
        })
        .err()
        .ok_or("Quiet Acquire renewed its cutoff")?;
    assert_eq!(cause.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(effects, 0);
    assert_eq!(pending.header, header);
    assert_eq!(pending.original, until);
    assert!(requests.outbound(header, until, client.now()?).is_err());
    assert!(requests.consumed(header).is_err());
    assert!(requests.received(header, client.now()?).is_err());
    drop(owner);
    Ok(())
}

/// Matching and foreign late inert replies keep their exact byte owner untouched.
#[test]
fn late_acquire_reply_cannot_transfer_matching_or_foreign_frame_at_or_after_cutoff()
-> std::result::Result<(), Box<dyn Error>> {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let clock = Arc::new(AcquireClock(AtomicU64::new(10)));
    let owner =
        sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), Arc::new(AcquireWake))?;
    let client = owner.client();
    let mut requests = requests()?;
    let until = Moment::from_nanos(1000);
    let header = requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    requests.outbound(header, until, client.now()?)?;
    requests.admitted(header)?;
    let pending = PendingAcquire { header, original: until, admitted: true };
    // Receive the original wire metadata before expiry; the semantic owner has not taken bytes.
    requests.received(header, client.now()?)?;
    let mut bytes = [0; 512];
    for (sample, observed) in
        [(1000, header), (1001, WireHeader { request: Some(RequestId::new(99)?), ..header })]
    {
        let count = FrameCodec::encode(
            &WireMessage {
                header: observed,
                payload: WirePayload::Reply(ReplyOutcome::Acknowledged),
            },
            &mut bytes,
        )?;
        let decoded = FrameCodec::decode(&bytes[..count])?;
        let mut retained = Some(&bytes[..count]);
        clock.0.store(sample, Ordering::Release);
        let cause = pending
            .take_matching(client.now()?, Some(decoded.header), || retained.take())
            .err()
            .ok_or("Late reply transferred its retained input")?;
        assert_eq!(cause.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(retained, Some(&bytes[..count]));
        assert_eq!(pending.header, header);
        assert_eq!(pending.original, until);
    }
    requests.consumed(header)?;
    drop(owner);
    Ok(())
}

/// Inert external reply loans exercise the actual Acquire custody selector. No native
/// peer/frame or successful write/readiness/retirement proof is constructed.
#[test]
fn retiring_acquire_retains_exact_original_loan_without_readmission_or_renewal()
-> std::result::Result<(), Box<dyn Error>> {
    let mut ledger = requests()?;
    let original = Moment::from_nanos(1000);
    let header = ledger.prepare(original, CounterDeadline::new(2000, 100)?)?;
    ledger.acquisition(header)?;
    ledger.outbound(header, original, Moment::from_nanos(10))?;
    ledger.admitted(header)?;
    let pending = PendingAcquire { header, original, admitted: true };
    let mut grant = None;
    let mut takes = 0;
    let foreign = WireHeader { deadline_qpc: header.deadline_qpc + 1, ..header };
    assert_eq!(
        pending.retain_retiring(&mut grant, Some(foreign), || {
            takes += 1;
            Some([7_u8, 8])
        })?,
        RetiringAcquireFrame::Awaiting
    );
    assert!(grant.is_none());
    assert_eq!(takes, 0);
    ledger.received(header, Moment::from_nanos(20))?;
    assert_eq!(
        pending.retain_retiring(&mut grant, Some(header), || {
            takes += 1;
            Some([7_u8, 8])
        })?,
        RetiringAcquireFrame::Transferred
    );
    assert_eq!(grant, Some([7, 8]));
    assert_eq!(
        pending.retain_retiring(&mut grant, None, || {
            takes += 1;
            Some([9_u8, 10])
        })?,
        RetiringAcquireFrame::Retained
    );
    assert_eq!(takes, 1);
    assert_eq!(grant, Some([7, 8]));
    assert_eq!(pending.original, original);
    ledger.progress(header, Moment::from_nanos(20))?;
    assert!(ledger.consumed(header).is_err());
    assert!(ledger.outbound(header, original, original).is_err());
    assert_eq!(grant, Some([7, 8]));
    assert_eq!(
        pending.retain_retiring(&mut grant, Some(header), || None)?,
        RetiringAcquireFrame::Retained
    );
    let mut absent: Option<[u8; 2]> = None;
    assert!(pending.retain_retiring(&mut absent, Some(header), || None).is_err());
    assert!(
        PendingAcquire { admitted: false, ..pending }
            .retain_retiring(&mut absent, Some(header), || Some([0, 0]))
            .is_err()
    );
    Ok(())
}
