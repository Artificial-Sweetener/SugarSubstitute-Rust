// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Closed origin projection retains strict v1 selection and exact full v2 data.
use super::*;
use sugarsubstitute_contracts::{
    crash::*, incident::*, lifecycle::*, recovery_supervisor::RecoveryGrantId,
};
#[test]
fn v2_origin_projection_separates_current_coordinator_historical_and_exact_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let operation = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?);
    assert!(
        RecoveryOrigin::from_origin(CaptureGenerationOrigin::Coordinator(owner), operation)?
            .is_none()
    );
    let current = LogicalStoreOperationKey::new(
        CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..owner },
        IncidentOperationId::new(2)?,
    );
    let historical =
        RecoveryOrigin::from_origin(CaptureGenerationOrigin::Coordinator(owner), current)?
            .ok_or("v2 missing")?;
    assert_eq!(historical.origin_kind, 1);
    assert_eq!(historical.root_epoch, *owner.epoch.bytes());
    assert_eq!(historical.capture_epoch, historical.root_epoch);
    assert_eq!(
        (historical.instance, historical.generation, historical.grant, historical.reserved),
        (0, 0, 0, 0)
    );
    let run = RunKey { owner, instance: InstanceId::new(4)?, generation: RunGeneration::new(5)? };
    let origin = CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(
        run,
        RecoveryGrantId::new(6)?,
    ));
    let recovered = RecoveryOrigin::from_origin(origin, current)?.ok_or("recovery v2 missing")?;
    assert_eq!(recovered.origin_kind, 2);
    assert_eq!((recovered.instance, recovered.generation, recovered.grant), (4, 5, 6));
    assert_eq!(recovered.capture_epoch, *origin.capture_key()?.epoch.bytes());
    assert_ne!(recovered.root_epoch, recovered.capture_epoch);
    assert_eq!(size_of::<RecoveryOrigin>(), 96);
    Ok(())
}
