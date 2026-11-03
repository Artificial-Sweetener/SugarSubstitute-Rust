// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real owned-input admission while an absent grant stays absent.
use super::*;
use crate::production::child::configuration::tests::{Rig, main_input};
use sugarsubstitute_contracts::{
    execution::JobOutcome,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
};

/// Missing native grant data reaches typed worker failure while original input stays charged.
#[test]
fn absent_grant_is_worker_state_failure_and_retains_actual_projected_input()
-> std::result::Result<(), Box<dyn Error + Send + Sync>> {
    let rig = Rig::new()?;
    let original = Moment::from_nanos(100);
    let mut projection = rig.client.submit_blocking(
        &rig.scope,
        configuration::request(JobId::new(1)?, original),
        move |context| main_input(&context),
    )?;
    let projected = rig.completion(&mut projection)?;
    let JobOutcome::Success(input) = projected.outcome else {
        return Err("Real input projection failed".into());
    };
    drop((projected.memory, projection));
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let identity = MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(1)? };
    let mut construction = Some(MainConstruction {
        frame: None,
        input,
        identity,
        header: WireHeader { run, connection: None, request: None, deadline_qpc: 100 },
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        settings: MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: sugarsubstitute_desktop::MainCaptionLabels {
                minimize: "minimize",
                maximize: "maximize",
                restore: "restore",
                close: "close",
            },
        },
        original,
        reloading: false,
    });
    let mut job = submit(&rig.client, &rig.scope, JobId::new(2)?, &mut construction)?;
    assert!(construction.is_none());
    assert_eq!(job.spec().operation, Operation::Infrastructure);
    assert_eq!(job.spec().class, JobClass::InfrastructureOrdinary);
    assert_eq!(job.spec().reservation_bytes, MainWindowRequest::RESERVATION_BYTES);
    assert_eq!(job.spec().deadline, Some(original));
    let completed = rig.completion(&mut job)?;
    let JobOutcome::Failure(failure) = completed.outcome else {
        return Err("Absent grant was accepted".into());
    };
    assert!(matches!(
        failure.source().and_then(|cause| cause.downcast_ref::<MainError>()),
        Some(MainError::State)
    ));
    let released = failure.release_frame();
    assert!(released.is_ok(), "Original missing frame did not settle: {released:?}");
    let alias = failure.clone();
    drop((completed.memory, job, failure));
    let Rig { owner, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, configuration::RESERVATION_BYTES);
    assert!(matches!(
        alias.source().and_then(|cause| cause.downcast_ref::<MainError>()),
        Some(MainError::State)
    ));
    drop(alias);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
