// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bad verdicts, trials without a verdict and abandoned trials through reloaded records.
use super::*;

const LATER: u64 = NOW + STRIKE_LIFETIME_SECONDS;

fn trial_of(record: &InstallationRecord, generation: &Generation) -> Result<InstallationRecord> {
    reload(&prepare(record, generation)?.begin_trial()?)
}

#[test]
fn one_bad_trial_rolls_back_but_only_a_second_unexpired_strike_withholds_the_release() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let accepted = install(&empty()?, &a)?;
    let once = strike(&accepted, &b, NOW)?;
    assert_eq!(once.rollback(), Some(&b));
    assert_eq!(once.startup_selection(), StartupSelection::Accepted(&a));
    assert!(!once.withheld(&b.package, NOW));
    assert!(!once.withheld_release(b.release.sequence(), NOW));
    assert!(once.prepare(b.clone(), PreparationIntent::Automatic, NOW).is_ok());
    let twice = strike(&once, &b, NOW + 60)?;
    assert_eq!(twice.strikes(), &[struck(&b, 2, NOW + 60)]);
    assert!(twice.withheld(&b.package, NOW + 60));
    assert!(twice.withheld(&b.package, LATER + 59));
    assert!(twice.withheld_release(b.release.sequence(), LATER + 59));
    assert!(!twice.withheld_release(b.release.sequence(), LATER + 60));
    assert!(!twice.withheld_release(generation(3)?.release.sequence(), NOW + 60));
    assert_eq!(
        twice.prepare(b.clone(), PreparationIntent::Automatic, LATER + 59),
        Err(Rejection::Quarantined)
    );
    assert!(twice.prepare(b.clone(), PreparationIntent::Repair, NOW + 60).is_ok());
    Ok(())
}

#[test]
fn withholding_expires_and_a_clock_set_before_the_strike_never_extends_it() -> Result {
    let b = generation(2)?;
    let twice = strike(&strike(&install(&empty()?, &generation(1)?)?, &b, NOW)?, &b, NOW)?;
    assert!(!twice.withheld(&b.package, LATER));
    assert!(twice.prepare(b.clone(), PreparationIntent::Automatic, LATER).is_ok());
    assert!(!twice.withheld(&b.package, NOW - 1));
    let restarted = strike(&twice, &b, LATER)?;
    assert_eq!(restarted.strikes(), &[struck(&b, 1, LATER)]);
    assert!(!restarted.withheld(&b.package, LATER));
    Ok(())
}

#[test]
fn a_trial_without_a_verdict_is_prepared_again_and_activates_on_the_next_launch() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let accepted = install(&empty()?, &a)?;
    let trial = trial_of(&accepted, &b)?;
    let released = reload(&trial.release_trial(&attempt(&trial)?)?)?;
    assert_eq!(released.candidate(), Some(&Candidate::Prepared { generation: b.clone() }));
    assert_eq!(released.startup_selection(), StartupSelection::Accepted(&a));
    assert!(released.strikes().is_empty());
    assert!(released.rollback().is_none());
    assert_eq!(released.release_trial(&attempt(&trial)?), Err(Rejection::Phase));
    let next_launch = reload(&released.begin_trial()?)?;
    let healthy = reload(&next_launch.accept(&attempt(&next_launch)?)?)?;
    assert_eq!(healthy.accepted(), Some(&b));
    assert_eq!(healthy.retained(), Some(&a));
    Ok(())
}

#[test]
fn abandoned_trials_retry_until_the_third_in_a_row_counts_as_one_strike() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let mut record = prepare(&install(&empty()?, &a)?, &b)?;
    for count in 1..UNWITNESSED_LIMIT {
        let trial = reload(&record.begin_trial()?)?;
        record = reload(&trial.abandon_trial(&attempt(&trial)?, NOW)?)?;
        assert_eq!(record.candidate(), Some(&Candidate::Prepared { generation: b.clone() }));
        assert_eq!(record.unwitnessed(), Some(&Unwitnessed { package: b.package.clone(), count }));
        assert!(record.strikes().is_empty());
    }
    let last = reload(&record.begin_trial()?)?;
    let third = reload(&last.abandon_trial(&attempt(&last)?, NOW)?)?;
    assert!(third.candidate().is_none());
    assert_eq!(third.rollback(), Some(&b));
    assert_eq!(third.strikes(), &[struck(&b, 1, NOW)]);
    assert!(third.unwitnessed().is_none());
    Ok(())
}

#[test]
fn an_observed_ending_breaks_the_abandoned_streak_and_another_package_starts_its_own() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let c = generation(3)?;
    let accepted = install(&empty()?, &a)?;
    let first = trial_of(&accepted, &b)?;
    let abandoned = reload(&first.abandon_trial(&attempt(&first)?, NOW)?)?;
    let second = reload(&abandoned.begin_trial()?)?;
    let observed = reload(&second.release_trial(&attempt(&second)?)?)?;
    assert!(observed.unwitnessed().is_none());
    let third = reload(&observed.begin_trial()?)?;
    let again = reload(&third.abandon_trial(&attempt(&third)?, NOW)?)?;
    assert_eq!(again.unwitnessed(), Some(&Unwitnessed { package: b.package.clone(), count: 1 }));
    let cancelled = again.cancel_preparation(&b)?;
    let other = reload(&cancelled.prepare(c.clone(), PreparationIntent::Automatic, NOW)?)?;
    assert!(other.unwitnessed().is_none());
    Ok(())
}

#[test]
fn acceptance_clears_the_release_strikes_and_streak_but_keeps_other_packages_strikes() -> Result {
    let a = generation(1)?;
    let b = generation(2)?;
    let c = generation(3)?;
    let both = strike(&strike(&install(&empty()?, &a)?, &c, NOW)?, &b, NOW)?;
    let trial = trial_of(&both, &b)?;
    let abandoned = reload(&trial.abandon_trial(&attempt(&trial)?, NOW)?)?;
    let retry = reload(&abandoned.begin_trial()?)?;
    let healthy = reload(&retry.accept(&attempt(&retry)?)?)?;
    assert_eq!(healthy.strikes(), &[struck(&c, 1, NOW)]);
    assert!(healthy.unwitnessed().is_none());
    assert!(healthy.rollback().is_none());
    Ok(())
}

#[test]
fn bad_counts_are_refused_while_fields_from_later_releases_are_ignored() -> Result {
    let b = generation(2)?;
    let trial = trial_of(&install(&empty()?, &generation(1)?)?, &b)?;
    let abandoned = reload(&trial.abandon_trial(&attempt(&trial)?, NOW)?)?;
    let one = strike(&abandoned.cancel_preparation(&b)?, &generation(3)?, NOW)?;
    let value: serde_json::Value = serde_json::from_slice(&abandoned.encode()?)?;
    let mut overlong = value.clone();
    overlong["unwitnessed"]["count"] = UNWITNESSED_LIMIT.into();
    let mut empty_streak = value;
    empty_streak["unwitnessed"]["count"] = 0.into();
    let value: serde_json::Value = serde_json::from_slice(&one.encode()?)?;
    let mut zero = value.clone();
    zero["strikes"][0]["count"] = 0.into();
    let mut duplicate = value.clone();
    let first = duplicate["strikes"][0].clone();
    duplicate["strikes"].as_array_mut().ok_or("Strikes absent")?.push(first);
    let mut later = value;
    later["strikes"][0]["reason"] = "crash".into();
    for hostile in [overlong, empty_streak, zero, duplicate] {
        assert_eq!(
            InstallationRecord::decode(&serde_json::to_vec(&hostile)?),
            Err(Rejection::Invariant)
        );
    }
    assert_eq!(InstallationRecord::decode(&serde_json::to_vec(&later)?)?, one);
    Ok(())
}
