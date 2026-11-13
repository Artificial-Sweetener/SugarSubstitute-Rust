// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual codec/handshake direction checks without inventing native peers.
use super::*;
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, Generation},
    lifecycle::{
        ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RequestId,
        RunGeneration, RunKey, UiChallengeId,
    },
    startup::{LaunchId, MainIdentity},
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FolderFragment, FrameCodec, RawWindowsUnits, ReplyOutcome, WireHeader,
};

type TestResult = Result<(), Box<dyn Error>>;

fn registered() -> Result<WireHandshake, Box<dyn Error>> {
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
    let mut bytes = [0; 512];
    let header = WireHeader {
        run,
        connection: Some(ConnectionId::new(19)?),
        request: None,
        deadline_qpc: 500,
    };
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
            payload: WirePayload::Reply(ReplyOutcome::Registered {
                backend_data_disposition:
                    sugarsubstitute_contracts::configuration::BackendDataDisposition::External,
            }),
        },
        &mut bytes,
    )?;
    binding.registered(&FrameCodec::decode(&bytes[..count])?)?;
    Ok(binding)
}
fn header(binding: &WireHandshake) -> Result<WireHeader, Box<dyn Error>> {
    Ok(WireHeader {
        run: binding.run(),
        connection: binding.connection(),
        request: Some(RequestId::new(24)?),
        deadline_qpc: 1000,
    })
}

#[test]
fn actual_environment_and_requested_folder_fragments_can_leave_the_child() -> TestResult {
    let binding = registered()?;
    let header = header(&binding)?;
    let main = MainIdentity {
        run: binding.run(),
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    let fragment = FolderFragment::new(main, 3, 0, RawWindowsUnits::from_units(&[69, 58, 92])?)?;
    for payload in
        [WirePayload::RequestedProfileFolder(fragment), WirePayload::EnvironmentPrepared(fragment)]
    {
        let message = WireMessage { header, payload };
        outbound(&binding, &message)?;
        let mut bytes = [0; 512];
        let count = FrameCodec::encode(&message, &mut bytes)?;
        assert_eq!(
            incoming(&binding, &FrameCodec::decode(&bytes[..count])?),
            Err(ApplicationWireError::Payload)
        );
    }
    Ok(())
}

#[test]
fn server_reply_can_enter_but_cannot_leave_as_a_client_operation() -> TestResult {
    let binding = registered()?;
    let message = WireMessage {
        header: header(&binding)?,
        payload: WirePayload::Reply(ReplyOutcome::Acknowledged),
    };
    assert_eq!(outbound(&binding, &message), Err(ApplicationWireError::Payload));
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(&message, &mut bytes)?;
    assert_eq!(incoming(&binding, &FrameCodec::decode(&bytes[..count])?), Ok(IncomingKind::Reply));
    Ok(())
}

#[test]
fn server_challenge_and_client_ui_response_use_their_original_directions() -> TestResult {
    let binding = registered()?;
    let challenge = UiChallengeId::new(41)?;
    let original = header(&binding)?;
    let message = WireMessage {
        header: WireHeader { request: None, ..original },
        payload: WirePayload::UiChallenge(challenge),
    };
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(&message, &mut bytes)?;
    assert_eq!(
        incoming(&binding, &FrameCodec::decode(&bytes[..count])?),
        Ok(IncomingKind::Challenge)
    );
    assert!(outbound(&binding, &message).is_err());
    let response = WireMessage { header: original, payload: WirePayload::UiResponse(challenge) };
    outbound(&binding, &response)?;
    let count = FrameCodec::encode(&response, &mut bytes)?;
    assert_eq!(
        incoming(&binding, &FrameCodec::decode(&bytes[..count])?),
        Err(ApplicationWireError::Payload)
    );
    Ok(())
}

#[test]
fn every_foreign_run_component_is_rejected_in_both_directions() -> TestResult {
    let binding = registered()?;
    let original = header(&binding)?;
    let mut foreign = [original.run; 4];
    foreign[0].owner.namespace = NamespaceId::new([3; 32]);
    foreign[1].owner.epoch = CoordinatorEpoch::new([4; 16])?;
    foreign[2].instance = InstanceId::new(8)?;
    foreign[3].generation = RunGeneration::new(10)?;
    for run in foreign {
        let header = WireHeader { run, ..original };
        assert_eq!(
            outbound(
                &binding,
                &WireMessage { header, payload: WirePayload::UiResponse(UiChallengeId::new(41)?) }
            ),
            Err(ApplicationWireError::Binding)
        );
        let mut bytes = [0; 512];
        let count = FrameCodec::encode(
            &WireMessage { header, payload: WirePayload::Reply(ReplyOutcome::Acknowledged) },
            &mut bytes,
        )?;
        assert_eq!(
            incoming(&binding, &FrameCodec::decode(&bytes[..count])?),
            Err(ApplicationWireError::Binding)
        );
    }
    outbound(
        &binding,
        &WireMessage {
            header: original,
            payload: WirePayload::UiResponse(UiChallengeId::new(41)?),
        },
    )?;
    Ok(())
}

#[test]
fn foreign_connection_never_matches_an_original_registered_reply() -> TestResult {
    let binding = registered()?;
    let header = WireHeader { connection: Some(ConnectionId::new(20)?), ..header(&binding)? };
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(
        &WireMessage { header, payload: WirePayload::Reply(ReplyOutcome::Acknowledged) },
        &mut bytes,
    )?;
    assert_eq!(
        incoming(&binding, &FrameCodec::decode(&bytes[..count])?),
        Err(ApplicationWireError::Binding)
    );
    assert_eq!(
        outbound(
            &binding,
            &WireMessage { header, payload: WirePayload::UiResponse(UiChallengeId::new(41)?) }
        ),
        Err(ApplicationWireError::Binding)
    );
    Ok(())
}

#[test]
fn missing_client_request_and_nonpositive_cutoff_never_reach_native_encoding() -> TestResult {
    let binding = registered()?;
    let original = header(&binding)?;
    for header in [
        WireHeader { request: None, ..original },
        WireHeader { deadline_qpc: 0, ..original },
        WireHeader { deadline_qpc: -1, ..original },
    ] {
        assert_eq!(
            outbound(
                &binding,
                &WireMessage { header, payload: WirePayload::UiResponse(UiChallengeId::new(41)?) }
            ),
            Err(ApplicationWireError::Binding)
        );
    }
    Ok(())
}
