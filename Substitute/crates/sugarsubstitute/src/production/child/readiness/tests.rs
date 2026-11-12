// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise cancellation and transport ordering with the real request ledger, without native proofs.
use super::*;
use crate::production::child::test_support::child;
use sugarsubstitute_contracts::lifecycle::ConnectionId;

#[test]
fn absent_prepared_startup_cannot_create_a_readiness_request() -> Result<()> {
    let (mut child, _owner, _) = child()?;
    assert!(child.report_main_revealed(child.identity, None).is_err());
    assert!(child.readiness.is_none());
    Ok(())
}

#[test]
fn closing_withdraws_unsent_readiness_without_blocking_the_next_control() -> Result<()> {
    let (mut child, _owner, _) = child()?;
    let client = child.client()?.clone();
    let identity = child.identity;
    let channel = child.bootstrap.channel_mut();
    channel.output_polling_fixture(&client, ConnectionId::new(1)?)?;
    let until = Moment::from_nanos(30_000_000_000);
    let header = channel.prepare_request(until)?;
    let later = channel.prepare_request(until)?;
    let message = WireMessage { header: later, payload: WirePayload::MainRetired(identity) };
    assert!(!channel.try_send(&message, until)?);
    child.readiness = Some(Readiness {
        identity: child.identity,
        header,
        until,
        admitted: false,
        acknowledged: false,
        frame: None,
    });
    child.begin_shutdown()?;
    child.advance_readiness()?;
    assert!(child.readiness.is_none());
    let channel = child.bootstrap.channel_mut();
    assert!(channel.request_prepared(header).is_err());
    assert!(channel.request_prepared(later)?);
    let cause = channel.try_send(&message, until).err().ok_or("Native transport absent")?;
    assert_eq!(cause.to_string(), "Lifecycle application session missing");
    child.advance_readiness()?;
    Ok(())
}

#[test]
fn contended_readiness_keeps_its_original_header_and_deadline() -> Result<()> {
    let (mut child, _owner, clock) = child()?;
    let client = child.client()?.clone();
    let channel = child.bootstrap.channel_mut();
    channel.output_polling_fixture(&client, ConnectionId::new(1)?)?;
    let until = Moment::from_nanos(30_000_000_000);
    let older = channel.prepare_request(until)?;
    let header = channel.prepare_request(until)?;
    child.readiness = Some(Readiness {
        identity: child.identity,
        header,
        until,
        admitted: false,
        acknowledged: false,
        frame: None,
    });
    for elapsed in [0, 1_000_000_000, 29_000_000_000] {
        clock.0.store(elapsed, std::sync::atomic::Ordering::SeqCst);
        child.advance_readiness()?;
        let pending = child.readiness.as_ref().ok_or("Original readiness lost")?;
        assert_eq!(pending.header, header);
        assert_eq!(pending.deadline(), Some(until));
        assert!(!pending.admitted);
    }
    clock.0.store(30_000_000_000, std::sync::atomic::Ordering::SeqCst);
    assert!(child.advance_readiness().is_err());
    assert_eq!(child.readiness.as_ref().ok_or("Original readiness lost")?.header, header);
    child.begin_shutdown()?;
    child.advance_readiness()?;
    assert!(child.bootstrap.channel_mut().request_prepared(older)?);
    Ok(())
}
