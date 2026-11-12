// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject actual ancestor closure independently of cancellation token state.

use super::{origin::RESERVATION_BYTES, test_support::HeldCapture};
use crate::runtime_test_support::{Rig, TestResult};
use sugarsubstitute_contracts::execution::*;

#[test]
fn ancestor_close_during_actual_provider_vetoes_first_bundle_issuance() -> TestResult {
    let rig = Rig::new()?;
    let child = rig
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&rig.scope))?;
    let held = HeldCapture::in_scope(&rig, &child, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let core = rig.client.core_for_test();
    let parent = rig.scope.id();
    rig.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(parent)));
    assert!(matches!(
        rig.client.capture_bundle_admission(&held.context),
        Err(AdmissionError::ScopeClosed { .. })
    ));
    assert!(rig.scope.snapshot().closed);
    assert!(!child.snapshot().closed);
    assert!(!held.context.cancellation_requested());
    assert!(held.context.memory().claim(RESERVATION_BYTES).is_ok());
    Ok(())
}

#[test]
fn actual_origin_fresh_effect_rejects_closed_ancestor_without_fabricated_cancellation() -> TestResult
{
    let rig = Rig::new()?;
    let child = rig
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&rig.scope))?;
    let held = HeldCapture::in_scope(&rig, &child, 1, RESERVATION_BYTES, 10_000_000_000)?;
    let origin = rig.client.capture_bundle_admission(&held.context)?.into_origin();
    assert!(origin.check_effect().is_ok());
    rig.client.core_for_test().close_scope(rig.scope.id());
    assert!(!held.context.cancellation_requested());
    assert!(matches!(origin.check_effect(), Err(AdmissionError::ScopeClosed { .. })));
    assert_eq!(child.snapshot().reserved_bytes, RESERVATION_BYTES);
    Ok(())
}
