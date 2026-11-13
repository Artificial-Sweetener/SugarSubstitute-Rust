// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual queued/issued receipt ordering through original acquisition expiry.
use super::*;

fn grant() -> Result<(ParentLifecycle, WireHeader, LeaseKey)> {
    super::super::acquisition_progress::receipt::unwritten_grant()
}

#[test]
fn unqueued_expired_grant_is_withdrawn_without_written_or_issued_fact() -> Result<()> {
    let (mut parent, h, _) = grant()?;
    assert!(parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    parent
        .expire_acquisition(Moment::from_nanos(180_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    assert!(!parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    assert!(parent.acquisition.as_ref().ok_or("Missing")?.retiring());
    assert!(
        parent
            .acquisition
            .as_mut()
            .ok_or("Missing")?
            .observe_write(h, WriteObservation::Written)
            .is_err()
    );
    assert!(!parent.closed && !parent.native_retired());
    Ok(())
}

#[test]
fn checked_revoked_unissued_final_grant_releases_hold_without_grant_authority() -> Result<()> {
    let (mut parent, h, lease) = grant()?;
    parent.acquisition.as_mut().ok_or("Missing")?.observe_write(h, WriteObservation::Queued)?;
    parent
        .expire_acquisition(Moment::from_nanos(180_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    assert!(parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    let receipt = super::super::super::write_receipt::WriteReceipt::new(7, 104);
    assert!(receipt.revoked(8).is_err());
    receipt.revoked(7)?;
    parent.acquisition.as_mut().ok_or("Missing")?.observe_write(h, WriteObservation::Revoked)?;
    assert!(!parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    assert!(
        parent
            .acquisition
            .as_ref()
            .ok_or("Missing")?
            .output_lease(copied_binding(lease), super::super::super::replies::reference(lease))
            .is_err()
    );
    assert!(
        parent
            .acquisition
            .as_mut()
            .ok_or("Missing")?
            .observe_write(h, WriteObservation::Written)
            .is_err()
    );
    Ok(())
}

#[test]
fn issued_original_write_cannot_revoke_or_settle_from_expiry() -> Result<()> {
    let (mut parent, h, _) = grant()?;
    let acquisition = parent.acquisition.as_mut().ok_or("Missing")?;
    acquisition.observe_write(h, WriteObservation::Queued)?;
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(7, 104);
    receipt.issue(7)?;
    acquisition.observe_write(h, WriteObservation::Issued)?;
    assert!(receipt.revoked(7).is_err());
    assert!(acquisition.observe_write(h, WriteObservation::Revoked).is_err());
    parent
        .expire_acquisition(Moment::from_nanos(180_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    assert!(parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    assert!(receipt.complete(7, 103).is_err());
    receipt.complete(7, 104)?;
    parent.acquisition.as_mut().ok_or("Missing")?.observe_write(h, WriteObservation::Written)?;
    assert!(!parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    assert!(parent.acquisition.as_ref().ok_or("Missing")?.retiring());
    assert!(!parent.native_retired());
    Ok(())
}

#[test]
fn timely_written_grant_keeps_running_lease_after_acquisition_cutoff() -> Result<()> {
    let (mut parent, h, lease) = grant()?;
    complete_acquisition_write(parent.acquisition.as_mut().ok_or("Missing")?, h)?;
    parent
        .expire_acquisition(Moment::from_nanos(999_000_000_000))
        .map_err(|e| e as Box<dyn Error>)?;
    assert!(!parent.acquisition.as_ref().ok_or("Missing")?.retiring());
    assert_eq!(parent.lease, Some(lease));
    assert!(parent.requests.next_deadline().is_some());
    Ok(())
}

#[test]
fn foreign_native_write_operation_and_header_cannot_close_original_state() -> Result<()> {
    let (mut parent, h, _) = grant()?;
    let mut receipt = super::super::super::write_receipt::WriteReceipt::new(7, 104);
    assert!(receipt.issue(8).is_err());
    parent.acquisition.as_mut().ok_or("Missing")?.observe_write(h, WriteObservation::Queued)?;
    let wrong = WireHeader { deadline_qpc: h.deadline_qpc + 1, ..h };
    assert_eq!(
        parent
            .acquisition
            .as_mut()
            .ok_or("Missing")?
            .observe_write(wrong, WriteObservation::Issued),
        Err(ParentError::ForeignPeer)
    );
    assert!(parent.hold_acquisition_receive().map_err(|e| e as Box<dyn Error>)?);
    Ok(())
}
