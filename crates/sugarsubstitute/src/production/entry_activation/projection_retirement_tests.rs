// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual failure/control allocations charged after passive source abandonment.
use super::*;
use crate::completion_wake::Wake;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[test]
fn unused_projection_source_return_keeps_first_cause_controls_and_original_charge_after_ack()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    // No authenticated loan/native authority is fabricated in this failure fixture.
    let mut input = Some(ProjectionInput::without_request());
    let data_bytes = projection_data_bytes().ok_or("Projection partition underflow")?;
    let mut job = client.submit_blocking_owned_box(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: PROJECTION_BYTES,
            replacement: None,
        },
        &mut input,
        move |mut retained, context| {
            retained.input_mut()._charge = match context.memory().claim(data_bytes) {
                Ok(claim) => Some(claim),
                Err(cause) => {
                    retained.input_mut().cause = ProjectionCause::Claim(cause);
                    return Err::<ProjectedEntry, _>(ProjectionFailure { retained });
                }
            };
            retained.input_mut().cause = ProjectionCause::Operation(Box::new(
                std::io::Error::other("original projection boundary cause"),
            ));
            retained.input_mut().post = Some(ControlFailure::Expired);
            Err::<ProjectedEntry, _>(ProjectionFailure { retained })
        },
    )?;
    let mut completed = wake.completion(&client, &mut job).map_err(|cause| cause.to_string())?;
    drop(job);
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    for _ in 0..2 {
        return_unused_source(&mut completed.outcome);
        let JobOutcome::Failure(failure) = &completed.outcome else {
            return Err("Original failure missing".into());
        };
        assert_eq!(
            failure.source().ok_or("First cause absent")?.to_string(),
            "original projection boundary cause"
        );
        assert!(matches!(failure.retained.input().post, Some(ControlFailure::Expired)));
        assert!(failure.retained.input()._charge.is_some());
        assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    }
    use super::super::{ProductionActivation, Record, RetainedRecord};
    use sugarsubstitute_contracts::{lifecycle::*, startup::*};
    let mut activation = ProductionActivation::new(
        r"E:\unopened\sugarsubstitute.exe".into(),
        r"E:\unopened".into(),
        r"D:\unopened".into(),
    )
    .map_err(|cause| std::io::Error::other(cause.to_string()))?;
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let mut record = Record::new(
        MainIdentity { run, launch: LaunchId::new(1)?, navigation: Generation::new(1)? },
        ConnectionId::new(1)?,
        Default::default(),
        false,
        false,
    );
    let storage = completed.memory.claim(size_of::<Record>() as u64)?;
    record.projected = Some(completed);
    activation.records[0] = Some(RetainedRecord { record: Box::new(record), _storage: storage });
    assert!(activation.backend.is_none());
    assert!(!activation.incidents.host.prepared());
    assert!(activation.check_initial_projection().is_err());
    assert_eq!(activation.failure_run, Some(run));
    assert_eq!(
        activation
            .failure_source()
            .and_then(Error::source)
            .ok_or("Original cause lost before incident initialization")?
            .to_string(),
        "original projection boundary cause"
    );
    drop(activation);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
