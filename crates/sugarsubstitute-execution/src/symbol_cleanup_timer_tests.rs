// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep expired, unactivated cleanup coordination deadlines out of the actual timer chooser.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::time::Duration;
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionPort, JobOutcome, ScopeId, ScopeSpec,
};

#[test]
fn symbol_expired_first_cleanup_token_keeps_same_held_job_without_expired_timer_polling()
-> TestResult {
    let rig = Rig::new()?;
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cleanup_spec = spec(1, JobClass::InfrastructureLifecycle, 32_768)?;
    let mut main_spec = spec(2, JobClass::InfrastructureOrdinary, 8_159_232)?;
    main_spec.deadline = Some(Moment::from_nanos(10_000_000_000));
    let permit = rig
        .client
        .reserve_symbol_cleanup(&cleanup, cleanup_spec, &rig.scope, main_spec, |_| Ok::<_, ()>(()))
        .map_err(|rejected| rejected.error)?;
    let mut main = rig
        .client
        .submit_symbol_helper(&rig.scope, main_spec, &permit, |_| Ok::<_, ()>(()))
        .map_err(|rejected| rejected.error)?;
    drop(rig.completion(&mut main)?);
    assert!(matches!(
        rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::ZERO),
        Err(AdmissionError::InvalidSymbolAdmission)
    ));
    assert!(matches!(
        rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(16)),
        Err(AdmissionError::InvalidSymbolAdmission)
    ));
    let token = rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15))?;
    rig.clock.advance(15_000_000_000);
    let rejected = match permit.begin(token) {
        Err(rejected) => rejected,
        Ok(_) => return Err("Expired cleanup token activated".into()),
    };
    assert!(
        matches!(rejected.error, AdmissionError::DeadlineExpired { deadline, .. } if deadline == token.deadline())
    );
    assert_eq!(rejected.permit.original_job_id(), cleanup_spec.id);
    assert_eq!(rejected.permit.status().phase, JobPhase::Queued);
    let state = lock(&rig.client.core.state);
    assert_eq!(deadlines(&state, token.deadline()), (None, false));
    drop(state);
    let mut actual = rejected.permit.cancel_and_observe();
    let completion = rig.completion(&mut actual)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(actual);
    drop(main);
    drop(rig.owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
