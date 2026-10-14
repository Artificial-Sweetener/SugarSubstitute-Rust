// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise authentic paired central jobs on the actual owned worker.
use crate::{
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn paired_originals_issue_while_both_running_and_deliver_distinct_credits() -> TestResult {
    let rig = Rig::new()?;
    let source_cleanup_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let receiving_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let receiving_cleanup_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?;
    let mut source_spec = spec(3, JobClass::InfrastructureOrdinary, 8_159_232)?;
    let mut receiving_spec = spec(4, JobClass::InfrastructureOrdinary, 131_072)?;
    source_spec.deadline = Some(Moment::from_nanos(10_000_000_000));
    receiving_spec.deadline = source_spec.deadline;
    let source_cleanup = rig
        .client
        .reserve_symbol_cleanup(
            &source_cleanup_scope,
            spec(1, JobClass::InfrastructureLifecycle, 32_768)?,
            &rig.scope,
            source_spec,
            |_| Ok::<_, u8>(true),
        )
        .map_err(|rejected| rejected.error)?;
    let receiving_cleanup = rig
        .client
        .reserve_report_export_cleanup(
            &receiving_cleanup_scope,
            spec(2, JobClass::InfrastructureLifecycle, 32_768)?,
            &receiving_scope,
            receiving_spec,
            |_| Ok::<_, u8>(true),
        )
        .map_err(|rejected| rejected.error)?;
    let client = rig.client.clone();
    let core = rig.client.core_for_test();
    let (mut source, mut receiving) = rig
        .client
        .submit_report_export_pair(
            crate::ReportExportPairRequest::new(
                &rig.scope,
                source_spec,
                &source_cleanup,
                &receiving_scope,
                receiving_spec,
                &receiving_cleanup,
            ),
            move |source, receiving| {
                {
                    let state = lock(&core.state);
                    assert_eq!(
                        state.jobs.get(&source.job_id()).map(|record| record.phase),
                        Some(JobPhase::Running)
                    );
                    assert_eq!(
                        state.jobs.get(&receiving.job_id()).map(|record| record.phase),
                        Some(JobPhase::Running)
                    );
                    assert!(
                        !MemoryLeaseObservation::new(source.memory())
                            .matches_lease(receiving.memory())
                    );
                }
                let reader =
                    client.committed_reader_admission(&source).map_err(|_| 1_u8)?.into_origin();
                let export =
                    client.report_export_admission(&receiving).map_err(|_| 2_u8)?.into_origin();
                reader.check_effect().map_err(|_| 3_u8)?;
                export.check_effect().map_err(|_| 4_u8)?;
                Ok::<_, u8>(true)
            },
        )
        .map_err(|rejected| rejected.error)?;
    let source_completion = rig.completion(&mut source)?;
    let receiving_completion = rig.completion(&mut receiving)?;
    assert!(matches!(source_completion.outcome, JobOutcome::Success(true)));
    assert!(matches!(receiving_completion.outcome, JobOutcome::Success(true)));
    assert_eq!(source_completion.spec.reservation_bytes, 8_159_232);
    assert_eq!(receiving_completion.spec.reservation_bytes, 131_072);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 8_159_232);
    assert_eq!(receiving_scope.snapshot().reserved_bytes, 131_072);
    drop(source_completion);
    drop(receiving_completion);
    let deadline = rig
        .client
        .freeze_symbol_cleanup_deadline(&source_cleanup, std::time::Duration::from_secs(15))?;
    let mut source_cleanup = source_cleanup.begin(deadline).map_err(|rejected| rejected.error)?;
    assert!(matches!(rig.completion(&mut source_cleanup)?.outcome, JobOutcome::Success(true)));
    let deadline = rig.client.freeze_report_export_cleanup_deadline(
        &receiving_cleanup,
        std::time::Duration::from_secs(15),
    )?;
    let mut receiving_cleanup =
        receiving_cleanup.begin(deadline).map_err(|rejected| rejected.error)?;
    assert!(matches!(rig.completion(&mut receiving_cleanup)?.outcome, JobOutcome::Success(true)));
    Ok(())
}
