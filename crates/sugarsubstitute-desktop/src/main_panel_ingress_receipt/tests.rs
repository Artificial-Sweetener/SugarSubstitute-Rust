// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove control-lane receipts retain exact navigation and original allocation custody.

use super::*;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Build a fully qualified native navigation without borrowing a window or global fixture.
fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        },
        launch: LaunchId::new(5)?,
        navigation: Generation::new(6)?,
    })
}

#[test]
fn control_receipt_preserves_exact_navigation_and_original_claim() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(PanelIngressReceipt::STORAGE_BYTES, Box::new(Release));
    let receipt = PanelIngressReceipt::control(identity, lease.claim(lease.bytes())?)?;
    assert_eq!(receipt.identity(), identity);
    assert_eq!(receipt.kind(), ReceiptKind::Control);
    assert!(lease.claim(1).is_err());
    let claim = receipt.into_claim();
    assert_eq!(claim.bytes(), PanelIngressReceipt::STORAGE_BYTES);
    assert!(lease.claim(1).is_err());
    drop(claim);
    assert!(lease.claim(lease.bytes()).is_ok());
    Ok(())
}

#[test]
fn cancellation_receipt_preserves_both_request_identity_boundaries() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(PanelIngressReceipt::STORAGE_BYTES, Box::new(Release));
    for request_id in [1, 2147483647] {
        let receipt =
            PanelIngressReceipt::cancellation(identity, request_id, lease.claim(lease.bytes())?)?;
        assert_eq!(receipt.identity(), identity);
        assert_eq!(receipt.kind(), ReceiptKind::Cancellation { request_id });
        assert!(lease.claim(1).is_err());
        drop(receipt);
        assert!(lease.claim(lease.bytes()).is_ok());
    }
    Ok(())
}

#[test]
fn invalid_receipt_inputs_do_not_retain_their_original_claim() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(PanelIngressReceipt::STORAGE_BYTES, Box::new(Release));
    assert!(matches!(
        PanelIngressReceipt::control(identity, lease.claim(lease.bytes() - 1)?),
        Err(PanelIngressReceiptError::Storage)
    ));
    assert!(matches!(
        PanelIngressReceipt::cancellation(identity, 1, lease.claim(lease.bytes() - 1)?),
        Err(PanelIngressReceiptError::Storage)
    ));
    for request_id in [0, 2147483648, u64::MAX] {
        assert!(matches!(
            PanelIngressReceipt::cancellation(identity, request_id, lease.claim(lease.bytes())?),
            Err(PanelIngressReceiptError::RequestIdentity)
        ));
        assert!(lease.claim(lease.bytes()).is_ok());
    }
    Ok(())
}
