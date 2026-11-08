// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual wire binding without fabricating native authentication.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration,
};
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

type TestResult = Result<(), Box<dyn Error>>;

fn run() -> Result<RunKey, Box<dyn Error>> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(9)?,
    })
}
fn handshake() -> Result<WireHandshake, Box<dyn Error>> {
    Ok(WireHandshake::new(run()?, CounterDeadline::new(500, 100)?))
}
fn welcome_header() -> Result<WireHeader, Box<dyn Error>> {
    Ok(WireHeader {
        run: run()?,
        connection: Some(ConnectionId::new(19)?),
        request: None,
        deadline_qpc: 500,
    })
}
fn reply_header() -> Result<WireHeader, Box<dyn Error>> {
    Ok(WireHeader { request: Some(RequestId::new(23)?), ..welcome_header()? })
}
fn encoded(header: WireHeader, payload: WirePayload<'_>) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = vec![0; 512];
    let count = FrameCodec::encode(&WireMessage { header, payload }, &mut bytes)?;
    bytes.truncate(count);
    Ok(bytes)
}
fn welcome(binding: &mut WireHandshake, header: WireHeader, frequency: i64) -> TestResult {
    let bytes = encoded(header, WirePayload::Welcome { qpc_frequency: frequency })?;
    binding.welcome(&FrameCodec::decode(&bytes)?)?;
    Ok(())
}
fn registered(
    binding: &mut WireHandshake,
    header: WireHeader,
    reply: ReplyOutcome<'_>,
) -> TestResult {
    let bytes = encoded(header, WirePayload::Reply(reply))?;
    binding.registered(&FrameCodec::decode(&bytes)?)?;
    Ok(())
}
fn pending_register() -> Result<WireHandshake, Box<dyn Error>> {
    let mut binding = handshake()?;
    binding.hello_admitted()?;
    welcome(&mut binding, welcome_header()?, 100)?;
    binding.register_admitted(RequestId::new(23)?)?;
    Ok(binding)
}

#[test]
fn original_run_counter_and_assigned_connection_survive_complete_exchange() -> TestResult {
    let mut binding = handshake()?;
    assert_eq!(
        binding.hello_header()?,
        WireHeader { run: run()?, connection: None, request: None, deadline_qpc: 500 }
    );
    assert_eq!(binding.connection(), None);
    binding.hello_admitted()?;
    welcome(&mut binding, welcome_header()?, 100)?;
    assert_eq!(binding.connection(), None);
    assert_eq!(binding.register_header(RequestId::new(23)?)?, reply_header()?);
    binding.register_admitted(RequestId::new(23)?)?;
    registered(
        &mut binding,
        reply_header()?,
        ReplyOutcome::Registered { backend_data_disposition: BackendDataDisposition::External },
    )?;
    assert_eq!(binding.connection(), Some(ConnectionId::new(19)?));
    assert_eq!(binding.run(), run()?);
    Ok(())
}

#[test]
fn unsolicited_welcome_cannot_bind_a_connection() -> TestResult {
    let mut binding = handshake()?;
    let bytes = encoded(welcome_header()?, WirePayload::Welcome { qpc_frequency: 100 })?;
    assert_eq!(binding.welcome(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Order));
    assert_eq!(binding.connection(), None);
    assert!(binding.hello_header().is_ok());
    Ok(())
}

#[test]
fn welcome_rejects_each_foreign_run_component_without_consuming_original() -> TestResult {
    for component in 0..4 {
        let mut binding = handshake()?;
        binding.hello_admitted()?;
        let mut header = welcome_header()?;
        match component {
            0 => header.run.owner.namespace = NamespaceId::new([3; 32]),
            1 => header.run.owner.epoch = CoordinatorEpoch::new([4; 16])?,
            2 => header.run.instance = InstanceId::new(8)?,
            _ => header.run.generation = RunGeneration::new(10)?,
        }
        let bytes = encoded(header, WirePayload::Welcome { qpc_frequency: 100 })?;
        assert_eq!(binding.welcome(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Identity));
        welcome(&mut binding, welcome_header()?, 100)?;
        assert_eq!(binding.register_header(RequestId::new(23)?)?, reply_header()?);
    }
    Ok(())
}

#[test]
fn welcome_cannot_shorten_or_extend_original_counter_deadline() -> TestResult {
    for deadline_qpc in [499, 501, i64::MAX] {
        let mut binding = handshake()?;
        binding.hello_admitted()?;
        let bytes = encoded(
            WireHeader { deadline_qpc, ..welcome_header()? },
            WirePayload::Welcome { qpc_frequency: 100 },
        )?;
        assert_eq!(binding.welcome(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Identity));
        welcome(&mut binding, welcome_header()?, 100)?;
    }
    Ok(())
}

#[test]
fn welcome_cannot_replace_enrolled_counter_frequency() -> TestResult {
    let mut binding = handshake()?;
    binding.hello_admitted()?;
    let bytes = encoded(welcome_header()?, WirePayload::Welcome { qpc_frequency: 101 })?;
    assert_eq!(binding.welcome(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Frequency));
    welcome(&mut binding, welcome_header()?, 100)?;
    Ok(())
}

#[test]
fn register_requires_welcome_and_can_be_admitted_only_once() -> TestResult {
    let mut binding = handshake()?;
    let request = RequestId::new(23)?;
    assert_eq!(binding.register_header(request), Err(HandshakeError::Order));
    binding.hello_admitted()?;
    assert_eq!(binding.hello_admitted(), Err(HandshakeError::Order));
    assert_eq!(binding.register_admitted(request), Err(HandshakeError::Order));
    welcome(&mut binding, welcome_header()?, 100)?;
    binding.register_admitted(request)?;
    assert_eq!(binding.register_admitted(request), Err(HandshakeError::Order));
    Ok(())
}

#[test]
fn acknowledgement_requires_actual_register_admission() -> TestResult {
    let mut binding = handshake()?;
    binding.hello_admitted()?;
    welcome(&mut binding, welcome_header()?, 100)?;
    let bytes = encoded(
        reply_header()?,
        WirePayload::Reply(ReplyOutcome::Registered {
            backend_data_disposition: BackendDataDisposition::External,
        }),
    )?;
    assert_eq!(binding.registered(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Order));
    binding.register_admitted(RequestId::new(23)?)?;
    registered(
        &mut binding,
        reply_header()?,
        ReplyOutcome::Registered { backend_data_disposition: BackendDataDisposition::External },
    )?;
    Ok(())
}

#[test]
fn acknowledgement_rejects_changed_run_connection_request_and_deadline() -> TestResult {
    for component in 0..4 {
        let mut binding = pending_register()?;
        let mut header = reply_header()?;
        match component {
            0 => header.run.generation = RunGeneration::new(10)?,
            1 => header.connection = Some(ConnectionId::new(20)?),
            2 => header.request = Some(RequestId::new(24)?),
            _ => header.deadline_qpc = 501,
        }
        let bytes = encoded(
            header,
            WirePayload::Reply(ReplyOutcome::Registered {
                backend_data_disposition: BackendDataDisposition::External,
            }),
        )?;
        assert_eq!(binding.registered(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Identity));
        assert_eq!(binding.connection(), None);
        registered(
            &mut binding,
            reply_header()?,
            ReplyOutcome::Registered { backend_data_disposition: BackendDataDisposition::External },
        )?;
    }
    Ok(())
}

#[test]
fn unsuccessful_register_does_not_publish_connection() -> TestResult {
    let mut binding = pending_register()?;
    let bytes = encoded(reply_header()?, WirePayload::Reply(ReplyOutcome::Cancelled))?;
    assert_eq!(binding.registered(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Payload));
    assert_eq!(binding.connection(), None);
    registered(
        &mut binding,
        reply_header()?,
        ReplyOutcome::Registered { backend_data_disposition: BackendDataDisposition::External },
    )?;
    Ok(())
}

#[test]
fn repeated_acknowledgement_preserves_registered_connection() -> TestResult {
    let mut binding = pending_register()?;
    registered(
        &mut binding,
        reply_header()?,
        ReplyOutcome::Registered { backend_data_disposition: BackendDataDisposition::External },
    )?;
    let bytes = encoded(
        reply_header()?,
        WirePayload::Reply(ReplyOutcome::Registered {
            backend_data_disposition: BackendDataDisposition::External,
        }),
    )?;
    assert_eq!(binding.registered(&FrameCodec::decode(&bytes)?), Err(HandshakeError::Order));
    assert_eq!(binding.connection(), Some(ConnectionId::new(19)?));
    Ok(())
}

#[test]
fn registration_requires_explicit_policy_and_preserves_both_native_dispositions() -> TestResult {
    for disposition in [BackendDataDisposition::External, BackendDataDisposition::InstallationOwned]
    {
        let mut binding = pending_register()?;
        assert_eq!(binding.backend_data_disposition(), None);
        let legacy = encoded(reply_header()?, WirePayload::Reply(ReplyOutcome::Acknowledged))?;
        assert_eq!(binding.registered(&FrameCodec::decode(&legacy)?), Err(HandshakeError::Payload));
        assert_eq!(binding.backend_data_disposition(), None);
        assert_eq!(binding.connection(), None);
        registered(
            &mut binding,
            reply_header()?,
            ReplyOutcome::Registered { backend_data_disposition: disposition },
        )?;
        assert_eq!(binding.backend_data_disposition(), Some(disposition));
        assert_eq!(binding.connection(), Some(ConnectionId::new(19)?));
    }
    Ok(())
}
