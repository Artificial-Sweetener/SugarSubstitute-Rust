// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe bounded original failures on real admitted, completed and acknowledged custody.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::io;
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, execution::*};

#[test]
fn repeated_fatal_post_observations_reuse_first_bounded_sources_until_last_alias() -> TestResult {
    let rig = Rig::new()?;
    let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut request = spec(1, JobClass::InfrastructureLifecycle, core_binding::PREPARATION_BYTES)?;
    request.operation = Operation::NativeLifecycle;
    request.deadline = Some(setup.until());
    let mut job = rig
        .client
        .submit_entry_preparation(&rig.scope, request, setup, Ok::<_, io::Error>)
        .map_err(|rejected| rejected.error)?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(success) = completion.outcome else {
        return Err("Actual sealed context missing".into());
    };
    drop(completion.memory);
    rig.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new("actual retained provider failure"))
    }));
    let first =
        success.value().observe_control().err().ok_or("Fatal actual provider was ignored")?;
    let first_cause = first.cause().ok_or("Original source missing")? as *const _;
    let mut retained = None;
    for _ in 0..24 {
        let next =
            success.value().observe_control().err().ok_or("Fatal post observation reopened")?;
        assert!(matches!(next.cause(), Some(EntryPreparationInputError::Control(_))));
        retained = Some(next);
    }
    let final_error = retained.ok_or("Last real observation missing")?;
    assert!(final_error.omitted_observations() >= 16);
    assert!(std::ptr::eq(final_error.cause().ok_or("Overflow lost first source")?, first_cause));
    drop(success);
    drop(first);
    drop(job);
    assert_eq!(rig.scope.snapshot().reserved_bytes, core_binding::PREPARATION_BYTES);
    let alias = final_error.clone();
    drop(final_error);
    assert_eq!(rig.scope.snapshot().reserved_bytes, core_binding::PREPARATION_BYTES);
    drop(alias);
    let Rig { owner, client, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(client);
    Ok(())
}

#[test]
fn compiled_closed_cause_storage_fits_unchanged_original_reservation() -> TestResult {
    let bound = layout::retained_native_bound().ok_or("Compiled layout overflow")?;
    assert!(bound < core_binding::PREPARATION_BYTES);
    assert!(bound > 100 * 1024 + 32767 * 2);
    Ok(())
}
