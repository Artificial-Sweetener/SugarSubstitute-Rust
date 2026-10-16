// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep authentic coordinator reservation/cutoff ownership while faking only loan availability.
use super::acquisition_progress::{complete_progress, copied_binding, pending};
use super::*;
use std::{cell::Cell, num::NonZeroU64};
use sugarsubstitute_contracts::lifecycle::RequestId;
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn shared_output_wait_keeps_original_request_unadmitted_until_capacity_returns() -> TestResult {
    let (mut parent, original, lease, _) = pending()?;
    complete_progress(&mut parent, original)?;
    parent.begin_output_sequence(lease)?;
    parent.connected = true;
    let reference = super::super::replies::reference(lease);
    let until = Moment::from_nanos(20_000_000_000);
    let request = RequestId::new(4)?;
    let header = WireHeader { request: Some(request), deadline_qpc: 20_000_000_000, ..original };
    let calls = Cell::new(0);
    for now in [Moment::from_nanos(1), Moment::from_nanos(19_999_999_999)] {
        assert!(
            parent
                .wait_output_loan(reference, None, until, now, |key| {
                    assert_eq!(key, lease.resource);
                    calls.set(calls.get() + 1);
                    Ok(false)
                })
                .map_err(|e| e as Box<dyn std::error::Error>)?
        );
        assert_eq!(parent.output_receive_until, Some(until));
        assert_eq!(parent.deadlines().min(), Some(until));
        assert!(!parent.receive_admission_open());
        assert!(parent.requests.find(request).is_none());
        assert!(parent.output.is_none());
        assert_eq!(parent.next_delivery, 0);
    }
    assert_eq!(calls.get(), 2);
    assert!(parent.requests.inspect(header, b"unchanged original pull")?.1);
    assert!(parent.requests.find(request).is_none());
    assert_eq!(
        parent.requests.inspect(original, b"changed acquisition").map_err(|e| e.code()),
        Err(ParentError::ChangedDuplicate)
    );
    assert!(
        !parent
            .wait_output_loan(reference, None, until, Moment::from_nanos(19_999_999_999), |_| Ok(
                true
            ))
            .map_err(|e| e as Box<dyn std::error::Error>)?
    );
    assert_eq!(parent.output_receive_until, None);
    assert!(parent.receive_admission_open());
    let (index, fresh) = parent.requests.admit(header, b"unchanged original pull")?;
    assert!(fresh);
    parent.requests.get_mut(index)?.until = Some(until);
    assert_eq!(parent.requests.admit(header, b"unchanged original pull")?, (index, false));
    assert!(!parent.requests.inspect(header, b"unchanged original pull")?.1);
    assert_eq!(
        parent.requests.inspect(header, b"changed pull").map_err(|e| e.code()),
        Err(ParentError::ChangedDuplicate)
    );
    assert_eq!(parent.requests.get(index)?.until, Some(until));
    assert!(parent.requests.get(index)?.reply.is_none());
    assert_eq!(parent.next_delivery, 0);
    Ok(())
}

#[test]
fn shared_output_wait_expiry_uses_original_retirement_and_preserves_other_rejections() -> TestResult
{
    let (mut parent, original, lease, _) = pending()?;
    complete_progress(&mut parent, original)?;
    parent.begin_output_sequence(lease)?;
    let reference = super::super::replies::reference(lease);
    let until = Moment::from_nanos(180_000_000_000);
    let supplied = Moment::from_nanos(181_000_000_000);
    assert!(
        parent
            .wait_output_loan(reference, None, supplied, Moment::from_nanos(1), |_| Ok(false))
            .map_err(|e| e as Box<dyn std::error::Error>)?
    );
    assert_eq!(parent.output_receive_until, Some(until));
    let failure = parent
        .wait_output_loan(reference, None, supplied, Moment::from_nanos(2), |_| {
            Err(ParentError::ForeignPeer.into())
        })
        .err()
        .ok_or("Capacity failure swallowed")?;
    assert_eq!(failure.downcast_ref::<ParentError>(), Some(&ParentError::ForeignPeer));
    let invented = NonZeroU64::new(99).ok_or("ACK absent")?;
    assert!(
        parent
            .wait_output_loan(reference, Some(invented), supplied, Moment::from_nanos(3), |_| Err(
                "Invalid ACK reached native capacity".into()
            ))
            .is_err()
    );
    assert!(
        !parent
            .wait_output_loan(reference, None, supplied, until, |_| Err(
                "Expired pull reached native capacity".into()
            ))
            .map_err(|e| e as Box<dyn std::error::Error>)?
    );
    assert_eq!(parent.output_receive_until, None);
    let header = WireHeader { request: Some(RequestId::new(4)?), ..original };
    let (index, _) = parent.requests.admit(header, b"original expired pull")?;
    assert!(
        parent
            .retire_expired_output(index, copied_binding(lease), reference, None, until, until)
            .map_err(|e| e as Box<dyn std::error::Error>)?
    );
    assert!(parent.requests.next_reply().is_none());
    assert_eq!(parent.requests.get(index)?.until, Some(until));
    assert!(parent.output.is_none());
    assert_eq!(parent.next_delivery, 0);
    Ok(())
}
