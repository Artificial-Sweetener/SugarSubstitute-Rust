// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Lock fixed native UI-policy units and nonwrapping challenge identities.
use std::{error::Error, time::Duration};
use sugarsubstitute_contracts::lifecycle::*;

#[test]
fn challenge_identity_rejects_zero_and_never_wraps() -> Result<(), Box<dyn Error>> {
    assert_eq!(UiChallengeId::new(0), Err(IdentityError::Zero));
    assert_eq!(UiChallengeId::new(1)?.next()?.get(), 2);
    assert_eq!(UiChallengeId::new(u64::MAX)?.next(), Err(IdentityError::Exhausted));
    Ok(())
}

#[test]
fn native_ui_cadence_and_initial_suspicion_have_fixed_second_units() {
    assert_eq!(UI_CHALLENGE_CADENCE, Duration::from_secs(5));
    assert_eq!(UI_SUSPICION_THRESHOLD, Duration::from_secs(30));
}
