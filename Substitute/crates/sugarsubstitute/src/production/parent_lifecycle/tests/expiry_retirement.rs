// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual Parent retirement owners, faking only native delivery boundary facts.
use super::super::acquisition::WriteObservation;
use super::acquisition_progress::{complete_progress, copied_binding, pending};
use super::*;
use sugarsubstitute_contracts::lifecycle::{LeaseKey, RequestOutcome};
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

#[path = "expiry_retirement/authorization.rs"]
mod authorization;
#[path = "expiry_retirement/native.rs"]
mod native;
#[path = "expiry_retirement/settlement.rs"]
mod settlement;
#[path = "expiry_retirement/write_rejection.rs"]
mod write_rejection;
#[path = "expiry_retirement/writes.rs"]
mod writes;

fn expired() -> Result<(Box<ParentLifecycle>, WireHeader, LeaseKey)> {
    use sugarsubstitute_contracts::lifecycle::{
        LifecycleFailure, LifecycleFailureCode, ResourceObservation,
    };
    let (mut parent, h, lease, mut coordinator) = pending()?;
    complete_progress(&mut parent, h)?;
    parent.begin_output_sequence(lease)?;
    let index = parent.requests.find(h.request.ok_or("Original absent")?).ok_or("Slot absent")?;
    let transition = coordinator
        .observe(ResourceObservation::Failed {
            resource: lease.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::other("original campaign preparation failure")),
            ),
        })
        .map_err(|rejected| {
            format!("Actual coordinator rejected failure: {:?}", rejected.reason)
        })?;
    let reply = transition
        .replies
        .iter()
        .find(|reply| Some(reply.request) == h.request)
        .ok_or("Actual Failed absent")?;
    assert_eq!(reply.outcome, RequestOutcome::Failed);
    assert_eq!(
        coordinator
            .failure(lease.resource)
            .and_then(Error::source)
            .ok_or("Cause lost")?
            .to_string(),
        "original campaign preparation failure"
    );
    parent.requests.observe_backend(index, reply.outcome)?;
    let acquisition = parent.acquisition.as_mut().ok_or("Acquisition absent")?;
    acquisition.observe_backend(h, reply.outcome)?;
    acquisition.reply(
        h,
        Reply::Failed(sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Preparation),
    )?;
    parent.requests.get_mut(index)?.reply = Some(Reply::Failed(
        sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Preparation,
    ));
    parent
        .expire_acquisition(Moment::from_nanos(180_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    Ok((Box::new(parent), h, lease))
}

fn admit_output(
    parent: &mut ParentLifecycle,
    h: WireHeader,
    lease: LeaseKey,
    ack: Option<std::num::NonZeroU64>,
) -> Result<(usize, Vec<u8>)> {
    let mut bytes = vec![0; 256];
    let count = FrameCodec::encode(
        &WireMessage {
            header: h,
            payload: WirePayload::OutputPull {
                lease: super::super::replies::reference(lease),
                acknowledged_delivery: ack,
            },
        },
        &mut bytes,
    )?;
    bytes.truncate(count);
    assert_eq!(FrameCodec::decode(&bytes)?.header, h);
    let (index, fresh) = parent.requests.admit(h, &bytes)?;
    assert!(fresh);
    Ok((index, bytes))
}

fn retirement_header(h: WireHeader) -> Result<WireHeader> {
    Ok(WireHeader { request: Some(RequestId::new(4)?), deadline_qpc: 195_000_000_000, ..h })
}

/// Receipt issue/completion are external fake delivery facts, not native capabilities.
fn acknowledge_retirement(parent: &mut ParentLifecycle, h: WireHeader) -> Result<()> {
    let mut receipt = super::super::write_receipt::WriteReceipt::new(9, 104);
    assert!(receipt.complete(9, 104).is_err());
    receipt.issue(9)?;
    receipt.complete(9, 104)?;
    parent
        .acquisition
        .as_mut()
        .ok_or("Original absent")?
        .observe_write(h, WriteObservation::Written)?;
    Ok(())
}

#[test]
fn exact_original_cutoff_fences_failed_reply_without_closing_or_discarding_acquisition()
-> Result<()> {
    let (mut parent, h, _) = expired()?;
    assert!(!parent.closed && parent.failure().is_none());
    assert!(
        parent
            .next_live_reply(Moment::from_nanos(180_000_000_000))
            .map_err(|e| e as Box<dyn Error>)?
            .is_none()
    );
    let index = parent.requests.find(h.request.ok_or("Original absent")?).ok_or("Slot absent")?;
    assert!(!parent.requests.get(index)?.sent);
    assert!(matches!(parent.requests.get(index)?.reply, Some(Reply::Failed(_))));
    assert_eq!(
        parent.acquisition.as_ref().ok_or("Missing")?.original,
        Moment::from_nanos(180_000_000_000)
    );
    assert!(!parent.native_retired());
    Ok(())
}

#[test]
fn authentic_expired_output_is_retired_without_output_effect_or_ordinary_ack() -> Result<()> {
    let (mut parent, h, lease) = expired()?;
    let output = WireHeader { deadline_qpc: h.deadline_qpc, ..retirement_header(h)? };
    let (index, bytes) = admit_output(&mut parent, output, lease, None)?;
    assert!(
        parent
            .retire_expired_output(
                index,
                copied_binding(lease),
                super::super::replies::reference(lease),
                None,
                Moment::from_nanos(180_000_000_000),
                Moment::from_nanos(180_000_000_001)
            )
            .map_err(|e| e as Box<dyn Error>)?
    );
    assert!(!parent.requests.get(index)?.sent);
    assert!(parent.requests.get(index)?.reply.is_none());
    assert_eq!(parent.requests.next_deadline(), None);
    assert_eq!(parent.next_delivery, 0);
    assert!(parent.output.is_none() && !parent.closed);
    assert_eq!(parent.requests.admit(output, &bytes)?, (index, false));
    assert_eq!(
        parent.requests.admit(output, b"changed output frame").map_err(|e| e.code()),
        Err(ParentError::ChangedDuplicate)
    );
    Ok(())
}

#[test]
fn foreign_source_wrong_ack_and_extended_output_cutoff_remain_strict_at_expiry() -> Result<()> {
    for fault in 0..4 {
        let (mut parent, h, lease) = expired()?;
        let output = WireHeader { deadline_qpc: h.deadline_qpc, ..retirement_header(h)? };
        let (index, _) = admit_output(&mut parent, output, lease, None)?;
        let mut binding = copied_binding(lease);
        let mut reference = super::super::replies::reference(lease);
        let mut ack = None;
        let mut until = Moment::from_nanos(180_000_000_000);
        match fault {
            0 => binding.connection = ConnectionId::new(2)?,
            1 => reference.lease = sugarsubstitute_contracts::lifecycle::LeaseId::new(2)?,
            2 => ack = std::num::NonZeroU64::new(1),
            _ => until = Moment::from_nanos(180_000_000_001),
        }
        assert!(
            parent
                .retire_expired_output(
                    index,
                    binding,
                    reference,
                    ack,
                    until,
                    Moment::from_nanos(180_000_000_001)
                )
                .is_err()
        );
        assert!(parent.requests.get(index)?.reply.is_none());
        assert_eq!(parent.next_delivery, 0);
    }
    Ok(())
}

#[test]
fn unrelated_expired_reply_is_not_downgraded_to_acquisition_retirement() -> Result<()> {
    let (mut parent, h, _) = expired()?;
    let (index, _) = parent.requests.admit(retirement_header(h)?, b"unrelated original command")?;
    parent.requests.get_mut(index)?.until = Some(Moment::from_nanos(180_000_000_000));
    parent.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
    assert!(parent.next_live_reply(Moment::from_nanos(180_000_000_001)).is_err());
    assert_eq!(parent.requests.next_reply(), Some(index));
    Ok(())
}

#[test]
fn queued_expired_output_reply_is_fenced_without_claiming_its_delivery_written() -> Result<()> {
    let (mut parent, h, lease) = expired()?;
    let (index, _) = parent.requests.admit(retirement_header(h)?, b"queued idle")?;
    parent.requests.acquisition(index, h.request.ok_or("Original absent")?)?;
    parent.requests.get_mut(index)?.until = Some(Moment::from_nanos(180_000_000_000));
    parent.requests.get_mut(index)?.reply = Some(Reply::OutputIdle(lease));
    assert!(
        parent
            .next_live_reply(Moment::from_nanos(180_000_000_001))
            .map_err(|e| e as Box<dyn Error>)?
            .is_none()
    );
    assert!(!parent.requests.get(index)?.sent);
    assert!(
        matches!(parent.requests.get(index)?.reply, Some(Reply::OutputIdle(actual)) if actual == lease)
    );
    Ok(())
}

#[test]
fn output_admission_at_exact_cutoff_rechecks_expiry_without_prior_activity_poll() -> Result<()> {
    let (mut parent, h, lease, _) = pending()?;
    complete_progress(&mut parent, h)?;
    parent.begin_output_sequence(lease)?;
    let output = WireHeader { deadline_qpc: h.deadline_qpc, ..retirement_header(h)? };
    let (index, _) = admit_output(&mut parent, output, lease, None)?;
    assert!(
        !parent
            .retire_expired_output(
                index,
                copied_binding(lease),
                super::super::replies::reference(lease),
                None,
                Moment::from_nanos(180_000_000_000),
                Moment::from_nanos(179_999_999_999)
            )
            .map_err(|e| e as Box<dyn Error>)?
    );
    assert!(
        parent
            .retire_expired_output(
                index,
                copied_binding(lease),
                super::super::replies::reference(lease),
                None,
                Moment::from_nanos(180_000_000_000),
                Moment::from_nanos(180_000_000_000)
            )
            .map_err(|e| e as Box<dyn Error>)?
    );
    assert!(parent.acquisition.as_ref().ok_or("Missing")?.retiring());
    assert!(parent.output.is_none() && parent.requests.get(index)?.reply.is_none());
    assert!(!parent.closed);
    Ok(())
}
