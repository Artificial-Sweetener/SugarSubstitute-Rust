// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove stable initial enrollment despite earlier completed process observations.
use crate::{
    native::{NativeCorrelation, NativeObservationError, ProcessExit, ProcessLease, Signal},
    runtime_test_support::{Gate, Rig, TestResult},
    scope::lock,
};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::*;

struct Lease {
    id: u64,
    order: Arc<Mutex<Vec<u64>>>,
    first_started: Arc<Gate>,
    first_release: Arc<Gate>,
    correlation: Option<NativeCorrelation>,
}
impl ProcessLease for Lease {
    fn start(
        &mut self,
        correlation: NativeCorrelation,
        signal: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        lock(&self.order).push(self.id);
        self.correlation = Some(correlation);
        if self.id == 1 {
            self.first_started.open();
            self.first_release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "fake_fifo_gate",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        signal.complete();
        Ok(())
    }
    fn read_exit(&mut self) -> Result<ProcessExit, NativeObservationError> {
        Ok(ProcessExit {
            correlation: self.correlation.ok_or_else(|| {
                NativeObservationError::new(
                    "fake_fifo_correlation",
                    std::io::Error::other("Missing actual start"),
                )
            })?,
            process_id: self.id as u32,
            creation_time_100ns: self.id,
            exit_code: 259,
        })
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        Ok(())
    }
}
#[test]
fn completed_earlier_entry_preserves_bound_later_initial_enrollment_order() -> TestResult {
    let rig = Rig::new()?;
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?;
    let order = Arc::new(Mutex::new(Vec::new()));
    let started = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let mut jobs = Vec::new();
    for id in 1..=4 {
        let spec = JobSpec {
            id: JobId::new(id)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 4096,
            replacement: None,
        };
        let permit =
            crate::native::reserve(&rig.client.core, &scope, spec, NativeCorrelation::new(id)?)?;
        jobs.push(permit.bind_input(Box::new(Lease {
            id,
            order: order.clone(),
            first_started: started.clone(),
            first_release: release.clone(),
            correlation: None,
        })));
        if id == 1 {
            started.wait()?;
        }
    }
    // All later inputs are bound before releasing the first native initialization.
    // On the next pass B/C/D are queued together; completing B must not enroll D first.
    release.open();
    for job in &mut jobs {
        assert!(matches!(
            rig.completion(job)?.outcome,
            JobOutcome::Success(ProcessExit { exit_code: 259, .. })
        ));
    }
    assert_eq!(*lock(&order), [1, 2, 3, 4]);
    Ok(())
}
