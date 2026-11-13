// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Genuine same-attempt settlement precedes ACK publication and correlation disposal.
use super::super::super::attempt::BackendRetirement;
use super::*;
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

fn prepared() -> Result<(Box<ParentLifecycle>, WireHeader, WireHeader, usize, LeaseKey)> {
    let (mut parent, h, lease) = expired()?;
    let retired = retirement_header(h)?;
    assert!(
        parent
            .observe_native_retirement(copied_binding(lease), identity()?, retired, false)
            .map_err(|e| e as Box<dyn Error>)?
    );
    acknowledge_retirement(&mut parent, retired)?;
    let settlement = WireHeader { request: Some(RequestId::new(5)?), ..retired };
    let mut bytes = [0; 256];
    let count = FrameCodec::encode(
        &WireMessage {
            header: settlement,
            payload: WirePayload::SettleAttempt {
                identity: identity()?,
                acquisition: h.request.ok_or("Original absent")?,
                sugarcubes_failed: false,
            },
        },
        &mut bytes,
    )?;
    let decoded = FrameCodec::decode(&bytes[..count])?;
    assert_eq!(decoded.header, settlement);
    let (index, _) = parent.requests.admit(settlement, &bytes[..count])?;
    parent.requests.get_mut(index)?.until = Some(Moment::from_nanos(195_000_000_000));
    let budget = parent
        .prepare_attempt_cleanup(
            index,
            identity()?,
            h.request.ok_or("Origin")?,
            Moment::from_nanos(195_000_000_000),
            Moment::from_nanos(180_000_000_001),
            false,
        )
        .map_err(|e| e as Box<dyn Error>)?;
    assert_eq!(budget.started, Moment::from_nanos(180_000_000_000));
    Ok((parent, h, settlement, index, lease))
}

#[test]
fn real_backend_boundary_must_release_exact_lease_before_staging_settlement_ack() -> Result<()> {
    let (mut parent, h, _, index, lease) = prepared()?;
    assert!(
        parent
            .observe_attempt_retirement(Moment::from_nanos(181_000_000_000), |actual| {
                assert_eq!(actual, Some(lease));
                Ok(BackendRetirement { cleanup_failed: false, released: false })
            })
            .map_err(|e| e as Box<dyn Error>)?
            .is_none()
    );
    assert!(parent.requests.get(index)?.reply.is_none());
    let ready = parent
        .observe_attempt_retirement(Moment::from_nanos(182_000_000_000), |actual| {
            assert_eq!(actual, Some(lease));
            Ok(BackendRetirement { cleanup_failed: false, released: true })
        })
        .map_err(|e| e as Box<dyn Error>)?
        .ok_or("Actual closed observation not accepted")?;
    parent.stage_settlement_reply(ready).map_err(|e| e as Box<dyn Error>)?;
    assert!(matches!(parent.requests.get(index)?.reply, Some(Reply::Acknowledged)));
    assert!(!parent.requests.get(index)?.sent);
    assert_eq!(parent.acquisition.as_ref().ok_or("Origin discarded before actual ACK")?.header, h);
    assert!(parent.attempt_cleanup.is_some());
    Ok(())
}

#[test]
fn matching_actual_settlement_write_retires_only_its_old_abandoned_correlation() -> Result<()> {
    let (mut parent, h, settlement, index, _) = prepared()?;
    let ready = parent
        .observe_attempt_retirement(Moment::from_nanos(182_000_000_000), |_| {
            Ok(BackendRetirement { cleanup_failed: false, released: true })
        })
        .map_err(|e| e as Box<dyn Error>)?
        .ok_or("Not ready")?;
    parent.stage_settlement_reply(ready).map_err(|e| e as Box<dyn Error>)?;
    assert!(parent.finish_attempt_write(settlement, Moment::from_nanos(183_000_000_000)).is_err());
    assert!(parent.acquisition.is_some());
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(9, 104);
    receipt.issue(9)?;
    receipt.complete(9, 104)?;
    // This metadata is set by the transport only after the checked actual receipt.
    parent.requests.get_mut(index)?.sent = true;
    parent
        .finish_attempt_write(settlement, Moment::from_nanos(183_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    assert!(parent.acquisition.is_none() && parent.attempt_cleanup.is_none());
    let old = parent.requests.find(h.request.ok_or("Origin")?).ok_or("Original history lost")?;
    assert!(!parent.requests.get(old)?.sent);
    assert_eq!(parent.requests.get(old)?.until, Some(Moment::from_nanos(180_000_000_000)));
    for number in 6..=20 {
        let next = WireHeader { request: Some(RequestId::new(number)?), ..settlement };
        let (slot, _) = parent.requests.admit(next, b"new observed operation")?;
        parent.requests.get_mut(slot)?.sent = true;
    }
    assert!(parent.requests.find(h.request.ok_or("Origin")?).is_none());
    assert!(!parent.native_retired());
    Ok(())
}

#[test]
fn cleanup_failure_or_original_equality_never_acknowledges_or_renews_settlement() -> Result<()> {
    let (mut parent, h, _, index, _) = prepared()?;
    assert!(
        parent
            .observe_attempt_retirement(Moment::from_nanos(182_000_000_000), |_| Ok(
                BackendRetirement { cleanup_failed: true, released: false }
            ))
            .map_err(|e| e as Box<dyn Error>)?
            .is_none()
    );
    assert!(matches!(
        parent.requests.get(index)?.reply,
        Some(Reply::Failed(sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Cleanup))
    ));
    assert!(parent.acquisition.is_some());
    assert_eq!(parent.acquisition.as_ref().ok_or("Original")?.header, h);
    let (mut parent, _, _, index, _) = prepared()?;
    assert!(
        parent
            .observe_attempt_retirement(Moment::from_nanos(195_000_000_000), |_| Err(
                "Expired gate invoked backend".into()
            ))
            .map_err(|e| e as Box<dyn Error>)?
            .is_none()
    );
    assert!(parent.requests.get(index)?.reply.is_none());
    assert_eq!(
        parent.attempt_cleanup.as_ref().ok_or("Attempt")?.until,
        Moment::from_nanos(195_000_000_000)
    );
    assert!(parent.acquisition.is_some() && !parent.closed);
    Ok(())
}

#[test]
fn stale_or_successor_settlement_identity_cannot_retire_current_expired_acquisition() -> Result<()>
{
    for fault in 0..2 {
        let (mut parent, h, lease) = expired()?;
        let retired = retirement_header(h)?;
        parent
            .observe_native_retirement(copied_binding(lease), identity()?, retired, false)
            .map_err(|e| e as Box<dyn Error>)?;
        acknowledge_retirement(&mut parent, retired)?;
        let settlement = WireHeader { request: Some(RequestId::new(5)?), ..retired };
        let (index, _) = parent.requests.admit(settlement, b"foreign settlement")?;
        let mut id = identity()?;
        let mut request = h.request.ok_or("Origin")?;
        if fault == 0 {
            id.launch = LaunchId::new(2)?;
        } else {
            request = RequestId::new(2)?;
        }
        assert!(
            parent
                .prepare_attempt_cleanup(
                    index,
                    id,
                    request,
                    Moment::from_nanos(195_000_000_000),
                    Moment::from_nanos(181_000_000_000),
                    false
                )
                .is_err()
        );
        assert!(parent.attempt_cleanup.is_none());
        assert_eq!(parent.acquisition.as_ref().ok_or("Origin")?.header, h);
    }
    Ok(())
}

#[test]
fn actual_settlement_write_at_original_cutoff_preserves_unsuccessful_attempt_custody() -> Result<()>
{
    let (mut parent, h, settlement, index, _) = prepared()?;
    let ready = parent
        .observe_attempt_retirement(Moment::from_nanos(182_000_000_000), |_| {
            Ok(BackendRetirement { cleanup_failed: false, released: true })
        })
        .map_err(|e| e as Box<dyn Error>)?
        .ok_or("Not ready")?;
    parent.stage_settlement_reply(ready).map_err(|e| e as Box<dyn Error>)?;
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(9, 104);
    receipt.issue(9)?;
    receipt.complete(9, 104)?;
    parent.requests.get_mut(index)?.sent = true;
    parent
        .finish_attempt_write(settlement, Moment::from_nanos(195_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    assert_eq!(parent.acquisition.as_ref().ok_or("Original custody discarded")?.header, h);
    assert_eq!(
        parent.attempt_cleanup.as_ref().ok_or("Attempt discarded")?.failed,
        Some(super::super::super::attempt::AttemptCleanupFailure::Deadline)
    );
    assert!(!parent.closed && !parent.native_retired());
    Ok(())
}
