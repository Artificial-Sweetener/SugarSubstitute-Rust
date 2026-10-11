// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume inherited timers once a retained pipe has selected native retirement.
use super::*;
use crate::{
    native,
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake, spec},
};
use sugarsubstitute_contracts::execution::*;

#[test]
fn stopped_pipe_removes_inherited_deadline_without_hiding_other_native_deadlines() -> TestResult {
    let core = Core::new(
        ExecutionId::new(1)?,
        Arc::new(ClockPort(Arc::new(ManualClock::default()))),
        Arc::new(Wake::default()),
    );
    let inherited = Moment::from_nanos(10);
    let scope = core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(1)?, deadline: Some(inherited) },
        None,
        true,
    )?;
    let mut specification = spec(1, JobClass::NativeObservation, 4096)?;
    specification.operation = Operation::NativeLifecycle;
    let permit = native::reserve(&core, &scope, specification, native::NativeCorrelation::new(1)?)?;
    let mut state = lock(&core.state);
    let record = state.jobs.get_mut(&JobId::new(1)?).ok_or("Missing admitted record")?;
    record.phase = JobPhase::Running;
    record.native_deadlines =
        Some(crate::native_deadlines::Deadlines::handshake(Moment::from_nanos(20)));
    assert_eq!(deadlines(&state, inherited), (None, true));
    state
        .jobs
        .get_mut(&JobId::new(1)?)
        .and_then(|r| r.native_deadlines.as_mut())
        .ok_or("Missing pipe deadlines")?
        .stop();
    for _ in 0..4 {
        assert_eq!(deadlines(&state, inherited), (None, false));
    }
    let record = state.jobs.get_mut(&JobId::new(1)?).ok_or("Missing retained record")?;
    assert_eq!(record.phase, JobPhase::Running);
    assert_eq!(record.deadline, Some(inherited));
    record.native_deadlines = None;
    assert_eq!(deadlines(&state, inherited), (None, true));
    state.stopping = true;
    assert_eq!(deadlines(&state, inherited), (None, false));
    drop(state);
    drop(permit);
    Ok(())
}
