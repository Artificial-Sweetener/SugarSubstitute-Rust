// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual fixed request ledger with deterministic protocol facts.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FrameCodec, ReplyOutcome, WireMessage, WirePayload,
};

type TestResult = Result<(), Box<dyn Error>>;

fn binding() -> Result<WireHandshake, Box<dyn Error>> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(9)?,
    };
    Ok(WireHandshake::new(run, CounterDeadline::new(500, 100)?))
}
fn requests() -> Result<ClientRequests, Box<dyn Error>> {
    requests_for_run(binding()?.run())
}
pub(in crate::production::child) fn requests_for_run(
    run: RunKey,
) -> Result<ClientRequests, Box<dyn Error>> {
    let mut binding = WireHandshake::new(run, CounterDeadline::new(500, 100)?);
    binding.hello_admitted()?;
    let header = WireHeader {
        run: binding.run(),
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
fn prepare(requests: &mut ClientRequests) -> Result<WireHeader, Box<dyn Error>> {
    Ok(requests.prepare(Moment::from_nanos(500), CounterDeadline::new(1000, 100)?)?)
}

#[test]
fn no_request_sequence_exists_before_exact_registration() -> TestResult {
    assert!(matches!(ClientRequests::new(&binding()?), Err(RequestError::Registration)));
    Ok(())
}

#[test]
fn first_application_identity_follows_the_actual_register_identity() -> TestResult {
    let mut requests = requests()?;
    let header = prepare(&mut requests)?;
    assert_eq!(header.request.map(RequestId::get), Some(24));
    assert_eq!(header.deadline_qpc, 1000);
    assert_eq!(header.run, binding()?.run());
    assert_eq!(header.connection, Some(ConnectionId::new(19)?));
    Ok(())
}

#[test]
fn frequency_rejection_consumes_neither_slot_nor_identity() -> TestResult {
    let mut requests = requests()?;
    assert_eq!(
        requests.prepare(Moment::from_nanos(500), CounterDeadline::new(1000, 101)?),
        Err(RequestError::Frequency)
    );
    assert_eq!(prepare(&mut requests)?.request.map(RequestId::get), Some(24));
    Ok(())
}

#[test]
fn capacity_retains_all_original_slots_and_does_not_consume_next_identity() -> TestResult {
    let mut requests = requests()?;
    let first = prepare(&mut requests)?;
    for _ in 1..REQUEST_SLOTS {
        prepare(&mut requests)?;
    }
    assert_eq!(
        prepare(&mut requests).err().and_then(|e| e.downcast_ref::<RequestError>().copied()),
        Some(RequestError::Capacity)
    );
    requests.admitted(first)?;
    requests.received(first, Moment::from_nanos(100))?;
    requests.consumed(first)?;
    assert_eq!(prepare(&mut requests)?.request.map(RequestId::get), Some(32));
    Ok(())
}

#[test]
fn busy_retries_keep_both_original_deadlines_and_request_identity() -> TestResult {
    let mut requests = requests()?;
    let header = prepare(&mut requests)?;
    for now in [100, 200, 499] {
        requests.outbound(header, Moment::from_nanos(500), Moment::from_nanos(now))?;
    }
    assert_eq!(
        requests.outbound(header, Moment::from_nanos(501), Moment::from_nanos(100)),
        Err(RequestError::Identity)
    );
    assert_eq!(
        requests.outbound(
            WireHeader { deadline_qpc: 1001, ..header },
            Moment::from_nanos(500),
            Moment::from_nanos(100)
        ),
        Err(RequestError::Identity)
    );
    requests.outbound(header, Moment::from_nanos(500), Moment::from_nanos(499))?;
    Ok(())
}

#[test]
fn reply_requires_actual_admission_and_retains_original_on_order_errors() -> TestResult {
    let mut requests = requests()?;
    let header = prepare(&mut requests)?;
    assert_eq!(requests.received(header, Moment::from_nanos(100)), Err(RequestError::Order));
    assert_eq!(requests.consumed(header), Err(RequestError::Order));
    requests.admitted(header)?;
    assert_eq!(requests.admitted(header), Err(RequestError::Order));
    assert_eq!(
        requests.outbound(header, Moment::from_nanos(500), Moment::from_nanos(100)),
        Err(RequestError::Order)
    );
    requests.received(header, Moment::from_nanos(100))?;
    requests.consumed(header)?;
    Ok(())
}

#[test]
fn foreign_reply_components_preserve_the_exact_original_request() -> TestResult {
    let mut requests = requests()?;
    let header = prepare(&mut requests)?;
    requests.admitted(header)?;
    let mut foreign_runs = [header.run; 4];
    foreign_runs[0].owner.namespace = NamespaceId::new([3; 32]);
    foreign_runs[1].owner.epoch = CoordinatorEpoch::new([4; 16])?;
    foreign_runs[2].instance = InstanceId::new(8)?;
    foreign_runs[3].generation = RunGeneration::new(10)?;
    for run in foreign_runs {
        assert_eq!(
            requests.received(WireHeader { run, ..header }, Moment::from_nanos(100)),
            Err(RequestError::Identity)
        );
    }
    for foreign in [
        WireHeader { connection: Some(ConnectionId::new(20)?), ..header },
        WireHeader { request: Some(RequestId::new(25)?), ..header },
        WireHeader { request: None, ..header },
        WireHeader { deadline_qpc: 999, ..header },
        WireHeader { deadline_qpc: 1001, ..header },
    ] {
        assert_eq!(
            requests.received(foreign, Moment::from_nanos(100)),
            Err(RequestError::Identity)
        );
    }
    requests.received(header, Moment::from_nanos(100))?;
    requests.consumed(header)?;
    Ok(())
}

#[test]
fn equality_and_later_cutoffs_reject_without_erasing_the_original_pair() -> TestResult {
    let mut requests = requests()?;
    let header = prepare(&mut requests)?;
    for now in [500, 501, u64::MAX] {
        assert_eq!(
            requests.outbound(header, Moment::from_nanos(500), Moment::from_nanos(now)),
            Err(RequestError::Deadline)
        );
    }
    requests.admitted(header)?;
    for now in [500, 501, u64::MAX] {
        assert_eq!(requests.received(header, Moment::from_nanos(now)), Err(RequestError::Deadline));
    }
    assert_eq!(requests.slots.iter().flatten().count(), 1);
    assert_eq!(requests.find(header)?.phase, Phase::Admitted);
    Ok(())
}

#[test]
fn duplicate_reply_and_consumption_do_not_displace_an_independent_request() -> TestResult {
    let mut requests = requests()?;
    let first = prepare(&mut requests)?;
    let second = prepare(&mut requests)?;
    requests.admitted(first)?;
    requests.admitted(second)?;
    requests.received(first, Moment::from_nanos(100))?;
    assert_eq!(requests.received(first, Moment::from_nanos(101)), Err(RequestError::Order));
    requests.consumed(first)?;
    assert_eq!(requests.consumed(first), Err(RequestError::Identity));
    requests.received(second, Moment::from_nanos(101))?;
    requests.consumed(second)?;
    Ok(())
}

#[test]
fn close_fences_new_work_and_preserves_original_received_evidence() -> TestResult {
    let mut requests = requests()?;
    let header = prepare(&mut requests)?;
    requests.admitted(header)?;
    requests.received(header, Moment::from_nanos(100))?;
    requests.close();
    assert_eq!(
        requests.prepare(Moment::from_nanos(500), CounterDeadline::new(1000, 100)?),
        Err(RequestError::Closed)
    );
    assert_eq!(
        requests.outbound(header, Moment::from_nanos(500), Moment::from_nanos(100)),
        Err(RequestError::Closed)
    );
    requests.consumed(header)?;
    Ok(())
}

#[test]
fn exhausted_sequence_never_wraps_or_overwrites_a_slot() -> TestResult {
    let mut requests = requests()?;
    requests.latest = u64::MAX - 1;
    assert_eq!(prepare(&mut requests)?.request.map(RequestId::get), Some(u64::MAX));
    for _ in 0..2 {
        assert_eq!(
            requests.prepare(Moment::from_nanos(500), CounterDeadline::new(1000, 100)?),
            Err(RequestError::Exhausted)
        );
    }
    assert_eq!(requests.slots.iter().flatten().count(), 1);
    Ok(())
}

#[test]
fn closed_prepared_withdrawal_keeps_exact_identity_and_nonwrapping_highwater() -> TestResult {
    let mut requests = requests()?;
    let first = prepare(&mut requests)?;
    let second = prepare(&mut requests)?;
    requests.close();
    let foreign = WireHeader { deadline_qpc: first.deadline_qpc + 1, ..first };
    assert_eq!(requests.withdraw_prepared(foreign), Err(RequestError::Identity));
    assert_eq!(requests.find(first)?.phase, Phase::Prepared);
    requests.withdraw_prepared(first)?;
    assert_eq!(requests.withdraw_prepared(first), Err(RequestError::Identity));
    assert_eq!(requests.find(second)?.phase, Phase::Prepared);
    assert_eq!(requests.latest, second.request.ok_or("Request absent")?.get());
    assert_eq!(requests.slots.iter().flatten().count(), 1);
    Ok(())
}

#[test]
fn prepared_withdrawal_cannot_erase_admitted_or_received_reply_custody() -> TestResult {
    let mut requests = requests()?;
    let first = prepare(&mut requests)?;
    requests.admitted(first)?;
    assert_eq!(requests.withdraw_prepared(first), Err(RequestError::Order));
    assert_eq!(requests.find(first)?.phase, Phase::Admitted);
    requests.received(first, Moment::from_nanos(100))?;
    requests.close();
    assert_eq!(requests.withdraw_prepared(first), Err(RequestError::Order));
    assert_eq!(requests.find(first)?.phase, Phase::Received);
    requests.consumed(first)?;
    assert_eq!(requests.slots.iter().flatten().count(), 0);
    Ok(())
}

#[test]
fn nonfinal_acquisition_keeps_original_slot_deadline_and_capacity_until_final() -> TestResult {
    let mut requests = requests()?;
    let until = Moment::from_nanos(1000);
    let h = requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    requests.acquisition(h)?;
    requests.admitted(h)?;
    requests.received(h, Moment::from_nanos(20))?;
    requests.progress(h, Moment::from_nanos(21))?;
    assert!(requests.consumed(h).is_err());
    assert!(requests.outbound(h, until, Moment::from_nanos(22)).is_err());
    assert!(requests.progress(h, Moment::from_nanos(22)).is_err());
    for _ in 0..7 {
        requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    }
    assert_eq!(
        requests.prepare(until, CounterDeadline::new(2000, 100)?),
        Err(RequestError::Capacity)
    );
    requests.received(h, Moment::from_nanos(23))?;
    requests.consumed(h)?;
    let next = requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    assert!(next.request > h.request);
    assert!(requests.received(h, Moment::from_nanos(24)).is_err());
    Ok(())
}
#[test]
fn progress_cannot_resume_foreign_nonacquire_expired_or_consumed_requests() -> TestResult {
    let mut requests = requests()?;
    let until = Moment::from_nanos(100);
    let control = requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    requests.admitted(control)?;
    requests.received(control, Moment::from_nanos(20))?;
    assert_eq!(requests.progress(control, Moment::from_nanos(21)), Err(RequestError::Order));
    requests.consumed(control)?;
    let h = requests.prepare(until, CounterDeadline::new(2000, 100)?)?;
    requests.acquisition(h)?;
    assert!(requests.acquisition(h).is_err());
    requests.admitted(h)?;
    requests.received(h, Moment::from_nanos(20))?;
    assert_eq!(
        requests
            .progress(WireHeader { deadline_qpc: h.deadline_qpc + 1, ..h }, Moment::from_nanos(21)),
        Err(RequestError::Identity)
    );
    assert_eq!(requests.progress(h, until), Err(RequestError::Deadline));
    requests.consumed(h)?;
    assert_eq!(requests.progress(h, Moment::from_nanos(21)), Err(RequestError::Identity));
    Ok(())
}

#[path = "requests/retirement_tests.rs"]
mod retirement_tests;
