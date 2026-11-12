// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A damaged accepted release falls back to the retained one and is prepared again, unpunished.
use super::*;

#[test]
fn a_damaged_release_runs_the_previous_one_without_a_strike_and_is_prepared_again() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let accepted = install(&install(&empty()?, &a)?, &b)?;
    let fallen = reload(&accepted.fall_back_to_retained(&b)?)?;
    assert_eq!(fallen.startup_selection(), StartupSelection::Accepted(&a));
    assert!(fallen.retained().is_none());
    assert_eq!(fallen.repair(), Some(&b));
    assert!(fallen.strikes().is_empty());
    assert!(fallen.rollback().is_none());
    assert!(!fallen.withheld_release(b.release.sequence(), NOW));
    // The same release in a fresh directory, re-downloaded and re-verified.
    let again = Generation { id: generation(3)?.id, ..b.clone() };
    assert_eq!(
        fallen.prepare(
            Generation { id: b.id.clone(), ..b.clone() },
            PreparationIntent::Automatic,
            NOW
        ),
        Err(Rejection::Invariant)
    );
    let repaired = install(&fallen, &again)?;
    assert_eq!(repaired.accepted(), Some(&again));
    assert_eq!(repaired.retained(), Some(&a));
    assert!(repaired.repair().is_none());
    Ok(())
}

#[test]
fn falling_back_drops_preparation_by_the_failed_engine_and_repeats_are_refused() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let accepted = install(&install(&empty()?, &a)?, &b)?;
    let pending = prepare(&accepted, &generation(4)?)?;
    let fallen = reload(&pending.fall_back_to_retained(&b)?)?;
    assert!(fallen.candidate().is_none());
    assert_eq!(fallen.revision(), pending.revision() + 1);
    assert_eq!(fallen.fall_back_to_retained(&b), Err(Rejection::Binding));
    assert_eq!(fallen.fall_back_to_retained(&a), Err(Rejection::Phase));
    Ok(())
}

#[test]
fn falling_back_needs_the_accepted_generation_a_retained_one_and_no_live_trial() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let first = install(&empty()?, &a)?;
    assert_eq!(first.fall_back_to_retained(&a), Err(Rejection::Phase));
    let accepted = install(&first, &b)?;
    assert_eq!(accepted.fall_back_to_retained(&a), Err(Rejection::Binding));
    let trial = reload(&prepare(&accepted, &generation(3)?)?.begin_trial()?)?;
    assert_eq!(trial.fall_back_to_retained(&b), Err(Rejection::Phase));
    assert_eq!(empty()?.fall_back_to_retained(&b), Err(Rejection::Binding));
    Ok(())
}
