// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test actual challenge/request owners with deterministic protocol facts.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RequestId, RunGeneration,
};
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn registered() -> Result<WireHandshake, Box<dyn Error>> {
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
    Ok(binding)
}
fn header(binding: &WireHandshake) -> WireHeader {
    WireHeader {
        run: binding.run(),
        connection: binding.connection(),
        request: None,
        deadline_qpc: 1234,
    }
}
fn receive(ui: &mut UiChallengeExchange, binding: &WireHandshake) -> TestResult {
    ui.receive(
        header(binding),
        UiChallengeId::new(41)?,
        Moment::from_nanos(100),
        Moment::from_nanos(0),
    )?;
    Ok(())
}
pub(super) fn processed(ui: &mut UiChallengeExchange, binding: &WireHandshake) -> TestResult {
    receive(ui, binding)?;
    let key = ui.dispatch(Moment::from_nanos(0))?.ok_or("No original native command")?;
    ui.processed(key, Moment::from_nanos(0))?;
    Ok(())
}

#[test]
fn receipt_and_command_delivery_cannot_substitute_for_actual_native_callback() -> TestResult {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let mut requests = ClientRequests::new(&binding)?;
    receive(&mut ui, &binding)?;
    assert!(ui.response(&mut requests, Moment::from_nanos(0))?.is_none());
    let key = ui.dispatch(Moment::from_nanos(0))?.ok_or("No command")?;
    assert!(ui.dispatch(Moment::from_nanos(0))?.is_none());
    assert!(ui.response(&mut requests, Moment::from_nanos(0))?.is_none());
    ui.processed(key, Moment::from_nanos(0))?;
    assert!(ui.response(&mut requests, Moment::from_nanos(0))?.is_some());
    Ok(())
}

#[test]
fn response_retries_preserve_server_ticks_local_cutoff_and_single_request_identity() -> TestResult {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let mut requests = ClientRequests::new(&binding)?;
    processed(&mut ui, &binding)?;
    let original = ui.response(&mut requests, Moment::from_nanos(1))?.ok_or("No response")?;
    assert_eq!(original.header.deadline_qpc, 1234);
    assert_eq!(original.header.request.map(RequestId::get), Some(24));
    assert_eq!(original.original, Moment::from_nanos(100));
    assert_eq!(original.id, UiChallengeId::new(41)?);
    assert_eq!(ui.response(&mut requests, Moment::from_nanos(99))?, Some(original));
    let next = requests.prepare(Moment::from_nanos(200), CounterDeadline::new(2222, 100)?)?;
    assert_eq!(next.request.map(RequestId::get), Some(25));
    requests.admitted(original.header)?;
    ui.admitted(original.header)?;
    assert!(ui.response(&mut requests, Moment::from_nanos(99))?.is_none());
    Ok(())
}

#[test]
fn foreign_binding_shape_or_ticks_leave_the_original_exchange_available() -> TestResult {
    let binding = registered()?;
    let original = header(&binding);
    let mut foreign_runs = [original.run; 4];
    foreign_runs[0].owner.namespace = NamespaceId::new([3; 32]);
    foreign_runs[1].owner.epoch = CoordinatorEpoch::new([4; 16])?;
    foreign_runs[2].instance = InstanceId::new(8)?;
    foreign_runs[3].generation = RunGeneration::new(10)?;
    let changed = foreign_runs.map(|run| WireHeader { run, ..original });
    for candidate in changed.into_iter().chain([
        WireHeader { connection: Some(ConnectionId::new(20)?), ..original },
        WireHeader { connection: None, ..original },
        WireHeader { request: Some(RequestId::new(1)?), ..original },
        WireHeader { deadline_qpc: 0, ..original },
    ]) {
        let mut ui = UiChallengeExchange::new(&binding)?;
        assert_eq!(
            ui.receive(
                candidate,
                UiChallengeId::new(41)?,
                Moment::from_nanos(100),
                Moment::from_nanos(0)
            ),
            Err(ChallengeError::Identity)
        );
        assert!(ui.deadline().is_none());
        receive(&mut ui, &binding)?;
        assert!(ui.dispatch(Moment::from_nanos(0))?.is_some());
    }
    Ok(())
}

#[test]
fn replay_and_overlap_cannot_replace_original_pending_native_command() -> TestResult {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    receive(&mut ui, &binding)?;
    for id in [40, 41] {
        assert_eq!(
            ui.receive(
                header(&binding),
                UiChallengeId::new(id)?,
                Moment::from_nanos(200),
                Moment::from_nanos(0)
            ),
            Err(ChallengeError::Replay)
        );
    }
    assert_eq!(
        ui.receive(
            header(&binding),
            UiChallengeId::new(42)?,
            Moment::from_nanos(200),
            Moment::from_nanos(0)
        ),
        Err(ChallengeError::Busy)
    );
    assert_eq!(ui.deadline(), Some(Moment::from_nanos(100)));
    assert_eq!(
        ui.dispatch(Moment::from_nanos(0))?.map(|key| key.id),
        Some(UiChallengeId::new(41)?)
    );
    Ok(())
}

#[test]
fn unsolicited_foreign_or_duplicate_native_callbacks_never_prepare_response() -> TestResult {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let key = UiChallengeKey { run: binding.run(), id: UiChallengeId::new(41)? };
    assert_eq!(ui.processed(key, Moment::from_nanos(0)), Err(ChallengeError::Order));
    receive(&mut ui, &binding)?;
    assert_eq!(ui.processed(key, Moment::from_nanos(0)), Err(ChallengeError::Order));
    assert_eq!(ui.dispatch(Moment::from_nanos(0))?, Some(key));
    let changed = UiChallengeKey { id: UiChallengeId::new(42)?, ..key };
    assert_eq!(ui.processed(changed, Moment::from_nanos(0)), Err(ChallengeError::Identity));
    ui.processed(key, Moment::from_nanos(0))?;
    assert_eq!(ui.processed(key, Moment::from_nanos(0)), Err(ChallengeError::Order));
    Ok(())
}

#[test]
fn equality_is_expired_at_receive_dispatch_callback_and_response() -> TestResult {
    let binding = registered()?;
    let until = Moment::from_nanos(100);
    let mut ui = UiChallengeExchange::new(&binding)?;
    assert_eq!(
        ui.receive(header(&binding), UiChallengeId::new(41)?, until, until),
        Err(ChallengeError::Deadline)
    );
    receive(&mut ui, &binding)?;
    assert_eq!(ui.dispatch(until), Err(ChallengeError::Deadline));
    let key = ui.dispatch(Moment::from_nanos(99))?.ok_or("No command")?;
    assert_eq!(ui.processed(key, until), Err(ChallengeError::Deadline));
    ui.processed(key, Moment::from_nanos(99))?;
    let mut requests = ClientRequests::new(&binding)?;
    let cause = ui.response(&mut requests, until).err().ok_or("Expired response accepted")?;
    assert_eq!(cause.downcast_ref::<ChallengeError>(), Some(&ChallengeError::Deadline));
    assert_eq!(ui.deadline(), Some(until));
    Ok(())
}

#[test]
fn exact_received_acknowledgement_releases_only_its_original_metadata() -> TestResult {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let mut requests = ClientRequests::new(&binding)?;
    processed(&mut ui, &binding)?;
    let PreparedUiResponse { header: response, .. } =
        ui.response(&mut requests, Moment::from_nanos(0))?.ok_or("No response")?;
    requests.admitted(response)?;
    ui.admitted(response)?;
    let independent =
        requests.prepare(Moment::from_nanos(200), CounterDeadline::new(2222, 100)?)?;
    requests.admitted(independent)?;
    requests.received(independent, Moment::from_nanos(0))?;
    assert!(!ui.matches_reply(independent));
    requests.consumed(independent)?;
    assert!(ui.acknowledged(&mut requests, response, ReplyOutcome::Acknowledged).is_err());
    assert!(ui.matches_reply(response));
    requests.received(response, Moment::from_nanos(99))?;
    ui.acknowledged(&mut requests, response, ReplyOutcome::Acknowledged)?;
    assert!(ui.deadline().is_none());
    assert!(requests.consumed(response).is_err());
    assert_eq!(
        ui.receive(
            header(&binding),
            UiChallengeId::new(41)?,
            Moment::from_nanos(200),
            Moment::from_nanos(0)
        ),
        Err(ChallengeError::Replay)
    );
    ui.receive(
        header(&binding),
        UiChallengeId::new(42)?,
        Moment::from_nanos(200),
        Moment::from_nanos(0),
    )?;
    Ok(())
}

#[test]
fn close_fences_every_phase_and_late_native_callback_without_erasing_pending_evidence() -> TestResult
{
    let binding = registered()?;
    for phase in 0..5 {
        let mut ui = UiChallengeExchange::new(&binding)?;
        let mut requests = ClientRequests::new(&binding)?;
        receive(&mut ui, &binding)?;
        let key = UiChallengeKey { run: binding.run(), id: UiChallengeId::new(41)? };
        if phase >= 1 {
            ui.dispatch(Moment::from_nanos(0))?;
        }
        if phase >= 2 {
            ui.processed(key, Moment::from_nanos(0))?;
        }
        if phase >= 3 {
            let PreparedUiResponse { header, .. } =
                ui.response(&mut requests, Moment::from_nanos(0))?.ok_or("No response")?;
            if phase == 4 {
                requests.admitted(header)?;
                ui.admitted(header)?;
            }
        }
        ui.close();
        assert!(ui.dispatch(Moment::from_nanos(0))?.is_none());
        ui.processed(key, Moment::from_nanos(0))?;
        assert!(ui.response(&mut requests, Moment::from_nanos(0))?.is_none());
        assert!(ui.deadline().is_none());
        assert!(ui.pending.is_some());
        assert_eq!(
            ui.receive(
                header(&binding),
                UiChallengeId::new(42)?,
                Moment::from_nanos(200),
                Moment::from_nanos(0)
            ),
            Err(ChallengeError::Closed)
        );
    }
    Ok(())
}
