// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise heartbeat I/O through the genuine admission, cancellation and retirement owners.
use super::*;
use sugarsubstitute_contracts::backend_custody::{
    BACKEND_CUSTODY_FRAME_BYTES, BackendCustodyCommand, BackendCustodyFrame,
};

fn encode(buffer: &mut LifecyclePipeWriteBuffer, command: BackendCustodyCommand) -> TestResult {
    let bytes = BackendCustodyFrame { key: key()?, nonce: 2, command }.encode()?;
    buffer.as_mut_bytes()[..BACKEND_CUSTODY_FRAME_BYTES].copy_from_slice(&bytes);
    Ok(())
}

#[test]
fn heartbeat_rejects_unauthenticated_or_nonheartbeat_messages_without_consuming_custody()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    session.control.inner.bind_input(PeerInput::Simulated);
    drop(fact(&rig, &mut session, |value| matches!(value, Fact::Connected))?);
    let mut buffer = session.control.take_write_buffer()?.ok_or("Write buffer missing")?;
    encode(&mut buffer, BackendCustodyCommand::Heartbeat)?;
    let rejection = session
        .control
        .write_heartbeat(LifecyclePipeOperationId::new(1)?, buffer)
        .err()
        .ok_or("Unauthenticated heartbeat accepted")?;
    assert_eq!(rejection.error, LifecyclePipeError::NotReady);
    drop(rejection.buffer);
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    drop(fact(&rig, &mut session, |value| matches!(value, Fact::Received { .. }))?);
    session.control.verify_capability([7; 32], Moment::from_nanos(900))?;
    drop(fact(&rig, &mut session, |value| matches!(value, Fact::Authenticated(_)))?);
    let mut buffer = session.control.take_write_buffer()?.ok_or("Returned buffer missing")?;
    encode(&mut buffer, BackendCustodyCommand::Finish { until_ticks: 100, frequency: 1 })?;
    let rejection = session
        .control
        .write_heartbeat(LifecyclePipeOperationId::new(2)?, buffer)
        .err()
        .ok_or("Ordinary command bypassed its deadline")?;
    assert_eq!(rejection.error, LifecyclePipeError::InvalidMessage);
    let mut buffer = rejection.buffer;
    encode(&mut buffer, BackendCustodyCommand::Heartbeat)?;
    session
        .control
        .write_heartbeat(LifecyclePipeOperationId::new(2)?, buffer)
        .map_err(|failure| failure.error)?;
    drop(fact(&rig, &mut session, |value| matches!(value, Fact::Written { .. }))?);
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn sleep_sized_clock_jump_does_not_expire_heartbeat_io_and_close_still_retires_it() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    authenticate(&rig, &mut session)?;
    boundary.pending_write.store(true, Ordering::Release);
    let mut buffer = session.control.take_write_buffer()?.ok_or("Write buffer missing")?;
    encode(&mut buffer, BackendCustodyCommand::Heartbeat)?;
    session
        .control
        .write_heartbeat(LifecyclePipeOperationId::new(2)?, buffer)
        .map_err(|failure| failure.error)?;
    drop(fact(&rig, &mut session, |value| matches!(value, Fact::WriteIssued { .. }))?);
    rig.clock.advance(3_600_000_000_000);
    rig.client.drain_completions(NonZeroUsize::MIN)?;
    assert!(session.take_terminal().is_none());
    assert_eq!(boundary.writes.load(Ordering::Acquire), 1);
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 1);
    Ok(())
}
