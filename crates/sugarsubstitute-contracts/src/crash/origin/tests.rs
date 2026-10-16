// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve legacy naming and separate every Root/run/grant component of recovery identity.
use super::*;
use crate::lifecycle::*;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn run() -> Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    })
}
fn name(origin: CaptureGenerationOrigin) -> Result<String> {
    let mut name = String::new();
    origin.write_name(&mut name)?;
    Ok(name)
}
#[test]
fn recovery_capture_origin_keeps_root_authority_and_legacy_bytes_while_separating_physical_generations()
-> Result {
    let run = run()?;
    let grant = RecoveryGrantId::new(5)?;
    let old = CaptureGenerationOrigin::Coordinator(run.owner);
    let fresh = CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(run, grant));
    assert_eq!(name(old)?, format!("capture-v1-{}{}", "01".repeat(32), "02".repeat(16)));
    assert_eq!(
        name(fresh)?,
        format!("capture-v1-{}7f28efa444ff878115e4b40d1ddecf4a", "01".repeat(32))
    );
    assert_eq!(old.coordinator(), fresh.coordinator());
    assert_ne!(name(old)?, name(fresh)?);
    assert_ne!(old.producer_subject()?, fresh.producer_subject()?);
    assert_ne!(fresh.capture_key()?, fresh.coordinator());
    assert!(fresh.contains(IncidentSubject::Desktop(run))?);
    assert!(fresh.contains(fresh.producer_subject()?)?);
    assert!(!fresh.contains(IncidentSubject::Supervisor(run.owner))?);
    assert!(!old.contains(fresh.producer_subject()?)?);
    assert_eq!(std::path::Path::new(&name(fresh)?).components().count(), 1);
    assert!(name(fresh)?.len() < 256);
    Ok(())
}
#[test]
fn recovery_capture_origin_rejects_every_substituted_root_run_or_grant_without_aliasing_names()
-> Result {
    let run = run()?;
    let grant = RecoveryGrantId::new(5)?;
    let origin = CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(run, grant));
    let mut variants = [run; 4];
    variants[0].owner.namespace = NamespaceId::new([9; 32]);
    variants[1].owner.epoch = CoordinatorEpoch::new([9; 16])?;
    variants[2].instance = InstanceId::new(9)?;
    variants[3].generation = RunGeneration::new(9)?;
    for changed in variants {
        assert!(!origin.contains(IncidentSubject::Desktop(changed))?);
        assert!(
            !origin.contains(
                CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(changed, grant))
                    .producer_subject()?
            )?
        );
        assert_ne!(
            name(origin)?,
            name(CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(changed, grant)))?
        );
    }
    assert_ne!(
        name(origin)?,
        name(CaptureGenerationOrigin::Recovery(RecoveryCaptureIdentity::new(
            run,
            RecoveryGrantId::new(6)?
        )))?
    );
    Ok(())
}
